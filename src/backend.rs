#[macro_use]
pub mod error;

pub mod handlers;
pub mod transports;

use self::error::BackendNiceError;
use self::transports::{Transport, TransportedValue};
use crate::impl_prelude::*;
use crate::project::{Comment, Fragment, Project, Translation};
use crate::utils::json::Value as JsonValue;
use crate::utils::{self, RcExt};

use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use serde::ser::SerializeSeq as _;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

pub const PROTOCOL_VERSION: u32 = 0;
pub static PROTOCOL_VERSION_STR: Lazy<String> = Lazy::new(|| PROTOCOL_VERSION.to_string());

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
enum MessageType {
  Request = 1,
  Response = 2,
  ErrorResponse = 3,
}

impl MessageType {
  fn from_int(n: u8) -> Option<Self> {
    Some(match n {
      1 => Self::Request,
      2 => Self::Response,
      3 => Self::ErrorResponse,
      _ => return None,
    })
  }
}

#[derive(Debug, Clone)]
pub enum Message {
  Request { id: Id, method: Cow<'static, str>, params: JsonValue },
  Response { id: Id, result: JsonValue },
  ErrorResponse { id: Option<Id>, message: Cow<'static, str> },
}

/// `u32`s are used because JS only has 32-bit integers. And 64-bit floats, but
/// those aren't really convenient for storing IDs.
pub type Id = u32;

impl<'de> Deserialize<'de> for Message {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    use serde::de::Error as DeError;

    struct MessageVisitor {}

    impl<'de> serde::de::Visitor<'de> for MessageVisitor {
      type Value = Message;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Message array")
      }

      #[inline]
      fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
      where
        A: serde::de::SeqAccess<'de>,
      {
        let msg_type = match seq.next_element::<u8>()? {
          Some(v) => v,
          None => return Err(DeError::invalid_length(0, &"message type")),
        };

        match MessageType::from_int(msg_type) {
          Some(MessageType::Request) => {
            let msg_id = match seq.next_element::<Id>()? {
              Some(v) => v,
              None => return Err(DeError::invalid_length(1, &"message ID")),
            };
            let method = match seq.next_element::<String>()? {
              Some(v) => v,
              None => return Err(DeError::invalid_length(2, &"request message method")),
            };
            let params = match seq.next_element::<JsonValue>()? {
              Some(v) => v,
              None => return Err(DeError::invalid_length(3, &"request message params")),
            };
            Ok(Message::Request { id: msg_id, method: Cow::Owned(method), params })
          }

          Some(MessageType::Response) => {
            let msg_id = match seq.next_element::<Id>()? {
              Some(v) => v,
              None => return Err(DeError::invalid_length(1, &"message ID")),
            };
            let result = match seq.next_element::<JsonValue>()? {
              Some(v) => v,
              None => return Err(DeError::invalid_length(2, &"response message result")),
            };
            Ok(Message::Response { id: msg_id, result })
          }

          Some(MessageType::ErrorResponse) => {
            let msg_id = match seq.next_element::<Option<Id>>()? {
              Some(v) => v,
              None => return Err(DeError::invalid_length(1, &"message ID")),
            };
            let err_msg = match seq.next_element::<String>()? {
              Some(v) => v,
              None => return Err(DeError::invalid_length(2, &"error response message text")),
            };
            Ok(Message::ErrorResponse { id: msg_id, message: Cow::Owned(err_msg) })
          }

          None => {
            return Err(DeError::invalid_value(
              serde::de::Unexpected::Unsigned(msg_type as u64),
              &"message type 1 <= i <= 3",
            ))
          }
        }
      }
    }

    deserializer.deserialize_seq(MessageVisitor {})
  }
}

impl Serialize for Message {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    match self {
      Message::Request { id, method, params } => {
        let mut seq = serializer.serialize_seq(Some(4))?;
        seq.serialize_element(&(MessageType::Request as u8))?;
        seq.serialize_element(&id)?;
        seq.serialize_element(&method)?;
        seq.serialize_element(&params)?;
        seq.end()
      }

      Message::Response { id, result } => {
        let mut seq = serializer.serialize_seq(Some(3))?;
        seq.serialize_element(&(MessageType::Response as u8))?;
        seq.serialize_element(&id)?;
        seq.serialize_element(&result)?;
        seq.end()
      }

      Message::ErrorResponse { id, message } => {
        let mut seq = serializer.serialize_seq(Some(3))?;
        seq.serialize_element(&(MessageType::ErrorResponse as u8))?;
        seq.serialize_element(&id)?;
        seq.serialize_element(&message)?;
        seq.end()
      }
    }
  }
}

pub trait Method: Sized + DeserializeOwned + 'static {
  fn name() -> &'static str;

  type Result: Sized + Serialize + 'static;

  fn handler(_backend: &mut Backend, _params: Self) -> AnyResult<Self::Result> {
    backend_nice_error!("the backend doesn't handle this request")
  }

  fn declaration() -> MethodDeclaration {
    MethodDeclaration {
      name: Self::name(),
      deserialize_request: |json| Ok(Box::new(serde_json::from_value::<Self>(json)?)),
      serialize_response: |any| serde_json::to_value(any.downcast::<Self::Result>().unwrap()),
      handle_call: |bk, any| Ok(Box::new(Self::handler(bk, *any.downcast::<Self>().unwrap())?)),
    }
  }
}

#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct MethodDeclaration {
  pub name: &'static str,
  pub deserialize_request: fn(JsonValue) -> serde_json::Result<Box<dyn Any>>,
  pub serialize_response: fn(Box<dyn Any>) -> serde_json::Result<JsonValue>,
  pub handle_call: fn(&'_ mut Backend, Box<dyn Any>) -> AnyResult<Box<dyn Any>>,
}

impl fmt::Debug for MethodDeclaration {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.debug_struct("MethodDeclaration")
      .field("name", &self.name)
      // <https://github.com/rust-lang/rust/blob/1.58.0/library/core/src/ptr/mod.rs#L1440-L1450>
      .field("deserialize_request", &(self.deserialize_request as usize as *const ()))
      .field("serialize_response", &(self.serialize_response as usize as *const ()))
      .field("handle_call", &(self.handle_call as usize as *const ()))
      .finish()
  }
}

inventory::collect!(MethodDeclaration);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FieldsSelection {
  #[serde(default)]
  fragments: Vec<FragmentField>,
  #[serde(default)]
  translations: Vec<TranslationField>,
  #[serde(default)]
  comments: Vec<CommentField>,
}

macro_rules! backend_fields_enum {
  ({$($tt:tt)+}) => { backend_fields_enum! { $($tt)+ } };

  (
    $(#[$enum_meta:meta])* $visibility:vis enum $enum_name:ident {
      $($(#[$variant_meta:meta])* $field_name:ident),+ $(,)?
    }
  ) => {
    #[derive(Debug, Clone, Copy, Deserialize, Serialize)]
    $(#[$enum_meta])*
    $visibility enum $enum_name {
      $(#[allow(non_camel_case_types)] $(#[$variant_meta])* $field_name,)+
    }

    impl $enum_name {
      $visibility const ALL: &'static [Self] = &[$(Self::$field_name),+];
    }
  };
}

backend_fields_enum!({
  pub enum FragmentField {
    id,
    tr_file_path,
    game_file_path,
    json_path,
    lang_uid,
    description,
    original_text,
    reference_texts,
    flags,
    translations,
    comments,
  }
});

#[derive(Debug, Clone)]
pub struct ListedFragment {
  pub fragment: Rc<Fragment>,
  pub select_fields: Rc<FieldsSelection>,
}

impl serde::Serialize for ListedFragment {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let fields = &self.select_fields.fragments;
    let mut seq = serializer.serialize_seq(Some(fields.len()))?;
    for field in fields.iter() {
      use FragmentField as F;
      let f = &self.fragment;
      match field {
        F::id => seq.serialize_element(&utils::serde::CompactUuidSerializer(f.id()))?,
        F::tr_file_path => seq.serialize_element(f.tr_file().relative_path())?,
        F::game_file_path => seq.serialize_element(f.virtual_game_file().path())?,
        F::json_path => seq.serialize_element(f.json_path())?,
        F::lang_uid => seq.serialize_element(&f.lang_uid())?,
        F::description => seq.serialize_element(f.description())?,
        F::original_text => seq.serialize_element(f.original_text())?,
        F::reference_texts => seq.serialize_element(&*f.reference_texts())?,
        F::flags => seq.serialize_element(&*f.flags())?,
        F::translations => seq.serialize_element(&utils::serde::SerializeSeqIterator::new(
          f.translations().iter().map(|t| ListedTranslation {
            translation: t.share_rc(),
            select_fields: self.select_fields.share_rc(),
          }),
        ))?,
        F::comments => seq.serialize_element(&utils::serde::SerializeSeqIterator::new(
          f.comments().iter().map(|c| ListedComment {
            comment: c.share_rc(),
            select_fields: self.select_fields.share_rc(),
          }),
        ))?,
      }
    }
    seq.end()
  }
}

backend_fields_enum!({
  pub enum TranslationField {
    id,
    author_username,
    editor_username,
    creation_timestamp,
    modification_timestamp,
    text,
    flags,
  }
});

#[derive(Debug, Clone)]
pub struct ListedTranslation {
  pub translation: Rc<Translation>,
  pub select_fields: Rc<FieldsSelection>,
}

impl serde::Serialize for ListedTranslation {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let fields = &self.select_fields.translations;
    let mut seq = serializer.serialize_seq(Some(fields.len()))?;
    for field in fields.iter() {
      use TranslationField as F;
      let t = &self.translation;
      match field {
        F::id => seq.serialize_element(&utils::serde::CompactUuidSerializer(t.id()))?,
        F::author_username => seq.serialize_element(t.author_username())?,
        F::editor_username => seq.serialize_element(&*t.editor_username())?,
        F::creation_timestamp => seq.serialize_element(&t.creation_timestamp())?,
        F::modification_timestamp => seq.serialize_element(&t.modification_timestamp())?,
        F::text => seq.serialize_element(&*t.text())?,
        F::flags => seq.serialize_element(&*t.flags())?,
      }
    }
    seq.end()
  }
}

backend_fields_enum!({
  pub enum CommentField {
    id,
    author_username,
    editor_username,
    creation_timestamp,
    modification_timestamp,
    text,
  }
});

#[derive(Debug, Clone)]
pub struct ListedComment {
  pub comment: Rc<Comment>,
  pub select_fields: Rc<FieldsSelection>,
}

impl serde::Serialize for ListedComment {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let fields = &self.select_fields.comments;
    let mut seq = serializer.serialize_seq(Some(fields.len()))?;
    for field in fields.iter() {
      use CommentField as F;
      let c = &self.comment;
      match field {
        F::id => seq.serialize_element(&utils::serde::CompactUuidSerializer(c.id()))?,
        F::author_username => seq.serialize_element(c.author_username())?,
        F::editor_username => seq.serialize_element(&*c.editor_username())?,
        F::creation_timestamp => seq.serialize_element(&c.creation_timestamp())?,
        F::modification_timestamp => seq.serialize_element(&c.modification_timestamp())?,
        F::text => seq.serialize_element(&*c.text())?,
      }
    }
    seq.end()
  }
}

#[derive(Debug)]
pub struct Backend {
  transport: Box<dyn Transport>,
  project_id_alloc: IdAllocator,
  projects: HashMap<Id, Rc<Project>>,
  methods_registry: HashMap<&'static str, &'static MethodDeclaration>,
}

impl Backend {
  pub fn new(transport: Box<dyn Transport>) -> Self {
    let mut methods_registry = HashMap::new();
    for decl in inventory::iter::<MethodDeclaration> {
      let decl: &MethodDeclaration = decl;
      if methods_registry.insert(decl.name, decl).is_some() {
        panic!("Duplicate method registered for name: {:?}", decl.name);
      }
    }

    Self {
      transport,
      project_id_alloc: IdAllocator::new(),
      // I assume that at least one project will be opened because otherwise
      // (without opening a project) the backend is pretty much useless
      projects: HashMap::with_capacity(1),
      methods_registry,
    }
  }

  pub fn start(&mut self) -> AnyResult<()> {
    let mut message_index: usize = 0;
    loop {
      match self.process_one_message(message_index) {
        Err(e) if e.is::<transports::TransportDisconnectionError>() => {
          info!("The frontend has disconnected, exiting cleanly");
          break;
        }
        Err(e) => {
          crate::report_error(e.context(format!("Failed to process message #{}", message_index)));
        }
        _ => {}
      }

      message_index = message_index.wrapping_add(1);
    }

    Ok(())
  }

  fn process_one_message(&mut self, message_index: usize) -> AnyResult<()> {
    let transported_value =
      self.transport.recv().context("Failed to receive message from the transport")?;
    let json_str: String;
    let in_msg: serde_json::Result<Message> = match transported_value {
      TransportedValue::Json(ref v) => serde_json::from_str(v),
      TransportedValue::Parsed(v) => try {
        json_str = serde_json::to_string(&v?)?;
        serde_json::from_str(&json_str)?
      }
      // TransportedValue::Parsed(Ok(v)) => Message::deserialize(v),
      // TransportedValue::Parsed(Err(v)) => Err(v),
    };

    let out_msg: Message = match in_msg {
      Ok(Message::Request { id, method, params }) => match self.process_request(method, params) {
        Ok(result) => Message::Response { id, result },
        Err(e) => {
          let mut message = "internal backend error".into();
          match e.downcast::<BackendNiceError>() {
            Ok(e) => {
              message = e.message;
              if let Some(e) = e.source {
                crate::report_error(e);
              }
            }
            Err(e) => {
              crate::report_error(e);
            }
          }
          Message::ErrorResponse { id: Some(id), message }
        }
      },

      Ok(Message::Response { .. }) | Ok(Message::ErrorResponse { .. }) => Message::ErrorResponse {
        id: None,
        message: "the backend currently can't receive responses".into(),
      },

      Err(e) if !e.is_io() => {
        warn!("Failed to deserialize message #{}: {}", message_index, e);
        Message::ErrorResponse { id: None, message: Cow::Owned(e.to_string()) }
      }

      Err(e) => {
        Err(e).context("Failed to deserialize message")?;
        unreachable!()
      }
    };

    let transported_value = if self.transport.uses_parsed_values() {
      let json_value = serde_json::to_value(&out_msg).context("Failed to serialize message")?;
      TransportedValue::Parsed(Ok(json_value))
    } else {
      let mut buf = Vec::new();
      serde_json::to_writer(&mut buf, &out_msg).context("Failed to serialize message")?;
      // Safe because serde_json doesn't emit invalid UTF-8, and besides JSON
      // files are required to be encoded as UTF-8 by the specification. See
      // <https://tools.ietf.org/html/rfc8259#section-8.1>.
      TransportedValue::Json(unsafe { String::from_utf8_unchecked(buf) })
    };

    self.transport.send(transported_value).context("Failed to send message to the transport")?;

    Ok(())
  }

  fn process_request(
    &mut self,
    method: Cow<'static, str>,
    params: JsonValue,
  ) -> AnyResult<JsonValue> {
    let method_decl: &'static MethodDeclaration = match self.methods_registry.get(&*method) {
      Some(v) => *v,
      None => backend_nice_error!("unknown method"),
    };
    let params = (method_decl.deserialize_request)(params)
      .context("Failed to deserialize message parameters")?;
    let result = (method_decl.handle_call)(self, params)?;
    let result =
      (method_decl.serialize_response)(result).context("Failed to serialize message result")?;
    Ok(result)
  }
}

#[derive(Debug, Clone)]
pub struct IdAllocator {
  current_id: Id,
  pub only_nonzero: bool,
  pub wrap_around: bool,
}

impl IdAllocator {
  pub fn new() -> Self { Self { current_id: 0, only_nonzero: true, wrap_around: true } }
}

impl Iterator for IdAllocator {
  type Item = Id;
  fn next(&mut self) -> Option<Self::Item> {
    // Clever branchless hack. Will take a max value with 1 when `only_nonzero`
    // is true, will not affect `self.next_id` otherwise.
    let id = self.current_id.max(self.only_nonzero as Id);
    let (next_id, overflow) = id.overflowing_add(1);
    if overflow && !self.wrap_around {
      None
    } else {
      self.current_id = next_id;
      Some(id)
    }
  }
}
