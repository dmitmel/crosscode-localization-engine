#[macro_use]
pub mod error;

pub mod handlers;
pub mod transports;

use self::error::BackendNiceError;
use self::transports::{Transport, TransportDisconnectionError};
use crate::impl_prelude::*;
use crate::logging;
use crate::project::{Comment, Fragment, Project, Translation};
use crate::utils::{self, ArcExt, RcExt};

use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use serde::ser::SerializeSeq as _;
use serde::{Deserialize, Serialize};
use serde_json::value::{to_raw_value, RawValue};
use std::any::Any;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub const PROTOCOL_VERSION: u32 = 0;
pub static PROTOCOL_VERSION_STR: Lazy<String> = Lazy::new(|| PROTOCOL_VERSION.to_string());

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
enum MessageType {
  Request = 1,
  Response = 2,
  ErrorResponse = 3,
  LogRecord = 4,
}

impl MessageType {
  fn from_int(n: u8) -> Option<Self> {
    Some(match n {
      1 => Self::Request,
      2 => Self::Response,
      3 => Self::ErrorResponse,
      4 => Self::LogRecord,
      _ => return None,
    })
  }
}

#[derive(Debug, Clone)]
pub enum Message<'a> {
  Request { id: Id, method: Cow<'a, str>, params: Cow<'a, RawValue> },
  Response { id: Id, result: Cow<'a, RawValue> },
  ErrorResponse { id: Option<Id>, message: Cow<'a, str> },
  LogRecord { level: log::Level, target: Cow<'a, str>, text: Cow<'a, str> },
}

impl<'a> Message<'a> {
  #[inline]
  pub fn response_id(&self) -> Option<Id> {
    match self {
      Self::Request { .. } => None,
      Self::Response { id, .. } => Some(*id),
      Self::ErrorResponse { id, .. } => *id,
      Self::LogRecord { .. } => None,
    }
  }
}

/// `u32`s are used because JS only has 32-bit integers. And 64-bit floats, but
/// those aren't really convenient for storing IDs.
pub type Id = u32;

impl<'de> Deserialize<'de> for Message<'de> {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    use serde::de::Error as DeError;

    struct MessageVisitor;

    impl<'de> serde::de::Visitor<'de> for MessageVisitor {
      type Value = Message<'de>;

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
            let params = match seq.next_element::<&'de RawValue>()? {
              Some(v) => v,
              None => return Err(DeError::invalid_length(3, &"request message params")),
            };
            Ok(Message::Request {
              id: msg_id,
              method: Cow::Owned(method),
              params: Cow::Borrowed(params),
            })
          }

          Some(MessageType::Response) => {
            let msg_id = match seq.next_element::<Id>()? {
              Some(v) => v,
              None => return Err(DeError::invalid_length(1, &"message ID")),
            };
            let result = match seq.next_element::<&'de RawValue>()? {
              Some(v) => v,
              None => return Err(DeError::invalid_length(2, &"response message result")),
            };
            Ok(Message::Response { id: msg_id, result: Cow::Borrowed(result) })
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

          Some(MessageType::LogRecord) => Err(DeError::custom(
            "deserialization of received LogRecord messages is not implemented",
          )),

          None => {
            return Err(DeError::invalid_value(
              serde::de::Unexpected::Unsigned(msg_type as u64),
              &"message type 1 <= i <= 4",
            ))
          }
        }
      }
    }

    deserializer.deserialize_seq(MessageVisitor)
  }
}

impl<'a> Serialize for Message<'a> {
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

      Message::LogRecord { level, target, text } => {
        let mut seq = serializer.serialize_seq(Some(4))?;
        seq.serialize_element(&(MessageType::LogRecord as u8))?;
        seq.serialize_element(&(*level as u8))?;
        seq.serialize_element(&target)?;
        seq.serialize_element(&text)?;
        seq.end()
      }
    }
  }
}

pub trait Method: Sized + DeserializeOwned + 'static {
  fn name() -> &'static str;

  type Result: Sized + Serialize + 'static;

  fn handler(backend: &mut Backend, params: Self) -> AnyResult<Self::Result>;

  fn declaration() -> MethodDeclaration {
    MethodDeclaration {
      client: false,
      name: Self::name(),
      deserialize_incoming: |json| Ok(Box::new(serde_json::from_str::<Self>(json.get())?)),
      serialize_outgoing: |any| to_raw_value(&any.downcast::<Self::Result>().unwrap()),
      handle_call: |bk, any| Ok(Box::new(Self::handler(bk, *any.downcast::<Self>().unwrap())?)),
    }
  }
}

pub trait ClientMethod: Sized + Serialize + 'static {
  fn name() -> &'static str;

  type Result: Sized + DeserializeOwned + 'static;

  fn declaration() -> MethodDeclaration {
    MethodDeclaration {
      client: true,
      name: Self::name(),
      deserialize_incoming: |json| Ok(Box::new(serde_json::from_str::<Self::Result>(json.get())?)),
      serialize_outgoing: |any| to_raw_value(&any.downcast::<Self>().unwrap()),
      handle_call: |_bk, _any| unreachable!("called handle_call on a client method"),
    }
  }
}

#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct MethodDeclaration {
  pub client: bool,
  pub name: &'static str,
  pub deserialize_incoming: fn(&RawValue) -> serde_json::Result<Box<dyn Any>>,
  pub serialize_outgoing: fn(Box<dyn Any>) -> serde_json::Result<Box<RawValue>>,
  pub handle_call: fn(&'_ mut Backend, Box<dyn Any>) -> AnyResult<Box<dyn Any>>,
}

impl fmt::Debug for MethodDeclaration {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.debug_struct("MethodDeclaration")
      .field("client", &self.client)
      .field("name", &self.name)
      // <https://github.com/rust-lang/rust/blob/1.58.0/library/core/src/ptr/mod.rs#L1440-L1450>
      .field("deserialize_incoming", &(self.deserialize_incoming as usize as *const ()))
      .field("serialize_outgoing", &(self.serialize_outgoing as usize as *const ()))
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
  transport: Arc<Mutex<Box<dyn Transport>>>,
  message_id: IdAllocator,
  project_id: IdAllocator,
  projects: HashMap<Id, Rc<Project>>,
  methods_registry: HashMap<&'static str, &'static MethodDeclaration>,
  client_methods_registry: HashMap<&'static str, &'static MethodDeclaration>,
  client_request_id: IdAllocator,
  client_responses: HashMap<Id, Message<'static>>,
  log_listener_id: usize,
}

impl Backend {
  pub fn new(transport: Box<dyn Transport>) -> Self {
    let mut methods_registry = HashMap::new();
    let mut client_methods_registry = HashMap::new();
    for decl in inventory::iter::<MethodDeclaration> {
      let decl: &MethodDeclaration = decl;
      let map = if decl.client { &mut client_methods_registry } else { &mut methods_registry };
      if map.insert(decl.name, decl).is_some() {
        panic!("Duplicate method registered for name: {:?}", decl.name);
      }
    }

    let transport = Arc::new(Mutex::new(transport));

    let log_listener_id = {
      let transport = transport.share_rc_weak();
      logging::ensure_installed();
      logging::add_listener(
        env_logger::filter::Builder::new().filter_level(log::LevelFilter::max()).build(),
        Box::new(move |record| {
          if let Some(transport) = transport.upgrade() {
            // Don't crash if the lock is poisoned, the main thread has most
            // certainly crashed and may have took down the transport with it,
            // thus there is no use sending a log message.
            if let Ok(transport) = transport.lock() {
              let _ = Self::send_one_message_to(&**transport, &Message::LogRecord {
                level: record.level(),
                target: Cow::Borrowed(record.target()),
                text: match record.args().as_str() {
                  Some(s) => Cow::Borrowed(s),
                  None => Cow::Owned(record.args().to_string()),
                },
              });
            }
          }
        }),
      )
    };

    Self {
      transport: transport.clone(),
      message_id: IdAllocator::new(),
      project_id: IdAllocator::new(),
      projects: HashMap::new(),
      methods_registry,
      client_methods_registry,
      client_request_id: IdAllocator::new(),
      client_responses: HashMap::new(),
      log_listener_id,
    }
  }

  pub fn start(&mut self) -> AnyResult<()> {
    match self.process_messages_until(None).unwrap_err() {
      TransportDisconnectionError => {
        // TODO: How can this message and other logger messages be sent after
        // disconnection of the transport?
        info!("The frontend has disconnected, exiting cleanly");
        Ok(())
      }
    }
  }

  fn process_messages_until(
    &mut self,
    expected_response_id: Option<Id>,
  ) -> Result<Message<'static>, TransportDisconnectionError> {
    if let Some(id) = expected_response_id {
      self.client_responses.remove(&id);
    }

    loop {
      let message_id = self.message_id.alloc();
      match self.process_one_message(message_id) {
        Ok(None) => {}

        Ok(Some(message)) => match message {
          Message::Request { .. } | Message::LogRecord { .. } => {
            unreachable!();
          }
          Message::ErrorResponse { id: None, message } => {
            warn!("Received an error from the client: {}", message);
          }
          Message::Response { id, .. } | Message::ErrorResponse { id: Some(id), .. } => {
            if Some(id) == expected_response_id {
              return Ok(message);
            } else {
              self.client_responses.insert(id, message);
            }
          }
        },

        Err(e) if e.is::<TransportDisconnectionError>() => {
          return Err(TransportDisconnectionError);
        }
        Err(e) => {
          crate::report_error!(e.context(format!("Failed to process message #{}", message_id)));
        }
      }

      if let Some(id) = expected_response_id {
        if let Some(message) = self.client_responses.remove(&id) {
          return Ok(message);
        }
      }
    }
  }

  fn process_one_message(&mut self, message_id: Id) -> AnyResult<Option<Message<'static>>> {
    let buf = {
      let transport = self.transport.lock().unwrap();
      transport.recv().context("Failed to receive message from the transport")?
    };
    match serde_json::from_str(&buf) {
      Err(e) => {
        warn!("Failed to deserialize message #{}: {}", message_id, e);
        self.send_one_message(&Message::ErrorResponse {
          id: None,
          message: Cow::Owned(e.to_string()),
        })?;
        Ok(None)
      }

      Ok(Message::Request { id, method, params }) => {
        let out_message = match self.process_request(method, params) {
          Ok(result) => Message::Response { id, result },
          Err(e) if e.is::<TransportDisconnectionError>() => {
            return Err(e);
          }
          Err(e) => match e.downcast::<BackendNiceError>() {
            Ok(e) => {
              if let Some(e) = e.source {
                crate::report_error!(e);
              }
              Message::ErrorResponse { id: Some(id), message: e.message }
            }
            Err(e) => {
              crate::report_error!(e);
              Message::ErrorResponse { id: Some(id), message: "internal backend error".into() }
            }
          },
        };
        self.send_one_message(&out_message)?;
        Ok(None)
      }

      Ok(Message::Response { id, result }) => {
        Ok(Some(Message::Response { id, result: Cow::Owned(result.into_owned()) }))
      }
      Ok(Message::ErrorResponse { id, message }) => {
        Ok(Some(Message::ErrorResponse { id, message: Cow::Owned(message.into_owned()) }))
      }

      Ok(Message::LogRecord { .. }) => Ok(None),
    }
  }

  fn send_one_message(&mut self, message: &Message) -> AnyResult<()> {
    let transport = self.transport.lock().unwrap();
    Self::send_one_message_to(&**transport, message)
  }

  fn send_one_message_to(transport: &dyn Transport, message: &Message) -> AnyResult<()> {
    let mut buf = Vec::new();
    serde_json::to_writer(&mut buf, message).context("Failed to serialize message")?;
    // Safe because serde_json doesn't emit invalid UTF-8, and besides JSON
    // files are required to be encoded as UTF-8 by the specification. See
    // <https://tools.ietf.org/html/rfc8259#section-8.1>.
    let buf = unsafe { String::from_utf8_unchecked(buf) };
    transport.send(buf).context("Failed to send message to the transport")?;
    Ok(())
  }

  // There must be explicit lifetimes here, so that the compiler understands
  // that the returned value doesn't pull a borrow for the whole `self`.
  fn process_request<'req, 'res>(
    &mut self,
    method: Cow<'req, str>,
    params: Cow<'req, RawValue>,
  ) -> AnyResult<Cow<'res, RawValue>> {
    let method_decl: &'static MethodDeclaration = match self.methods_registry.get(&*method) {
      Some(v) => *v,
      None => backend_nice_error!("unknown method"),
    };
    let params = (method_decl.deserialize_incoming)(&*params)
      .context("Failed to deserialize message parameters")?;
    let result = (method_decl.handle_call)(self, params)?;
    let result =
      (method_decl.serialize_outgoing)(result).context("Failed to serialize message result")?;
    Ok(Cow::Owned(result))
  }

  pub fn send_request<M: ClientMethod>(&mut self, params: M) -> AnyResult<M::Result> {
    let method_decl: &'static MethodDeclaration =
      self.client_methods_registry.get(M::name()).unwrap();
    let params = (method_decl.serialize_outgoing)(Box::new(params))
      .context("Failed to serialize client message parameters")?;
    let result = self.send_request_impl(Cow::Borrowed(M::name()), Cow::Owned(params))?;
    let result = (method_decl.deserialize_incoming)(&*result)
      .context("Failed to deserialize client message result")?;
    Ok(*result.downcast::<M::Result>().unwrap())
  }

  // Same story with explicit lifetimes as in process_request.
  pub fn send_request_impl<'req, 'res>(
    &mut self,
    method: Cow<'req, str>,
    params: Cow<'req, RawValue>,
  ) -> AnyResult<Cow<'res, RawValue>> {
    let id = self.client_request_id.alloc();
    self.send_one_message(&Message::Request { id, method, params })?;
    match self.process_messages_until(Some(id))? {
      Message::Request { .. } | Message::LogRecord { .. } => unreachable!(),
      Message::Response { result, .. } => Ok(result),
      Message::ErrorResponse { message, .. } => bail!("client error: {}", message),
    }
  }
}

impl Drop for Backend {
  fn drop(&mut self) { logging::remove_listener(self.log_listener_id); }
}

#[derive(Debug, Clone)]
pub struct IdAllocator {
  current: Id,
}

impl IdAllocator {
  #[inline(always)]
  pub fn new() -> Self { Self { current: 0 } }

  #[inline]
  pub fn alloc(&mut self) -> Id {
    let id = self.current.max(1);
    self.current = id.wrapping_add(1);
    id
  }
}

impl Iterator for IdAllocator {
  type Item = Id;
  #[inline(always)]
  fn next(&mut self) -> Option<Self::Item> { Some(self.alloc()) }
}
