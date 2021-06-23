use crate::impl_prelude::*;
use crate::rc_string::RcString;
use crate::utils;
use crate::utils::json::{self, Value};

use std::borrow::Cow;
use std::str::FromStr;

#[derive(Debug)]
struct GeneratorState<'json> {
  file_data: &'json json::Value,
  description: Vec<RcString>,
  words: Vec<Cow<'json, str>>,
  current_entity_type: Option<&'json str>,
}

// Rewritten from <https://github.com/dmitmel/crosscode-ru/blob/ea8ee6244d0c89e3118f2344440181f594d95783/tool/src/Notabenoid.ts#L499-L667>
// LMT's tagging algorithm also looks interesting, maybe we can learn something from it as well:
// <https://github.com/L-Sherry/Localize-Me-Tools/blob/cb8863cef80d1c7361b7142ab9206226e9669bdf/tags.py>
pub fn generate(file_data: &json::Value, fragment_json_path: &str) -> AnyResult<Vec<RcString>> {
  let mut state = GeneratorState {
    file_data,
    description: Vec::new(),
    words: Vec::new(),
    current_entity_type: None,
  };

  let mut current_value = file_data;
  for (depth, key) in fragment_json_path.split('/').enumerate() {
    let step = || -> Option<&json::Value> {
      match current_value {
        json::Value::Object(object) => object.get(key),
        json::Value::Array(array) => array.get(usize::from_str(key).ok()?),
        _ => None,
      }
    };
    let next_value =
      step().ok_or_else(|| format_err!("Invalid JSON path at depth {}", depth + 1))?;
    generate_for_json_object(current_value, key, &mut state);
    current_value = next_value;
  }

  Ok(state.description)
}

fn generate_for_json_object<'json>(
  value: &'json json::Value,
  key: &'json str,
  state: &mut GeneratorState<'json>,
) -> Option<()> {
  state.words.clear();
  let object = value.as_object()?;

  if state.current_entity_type == Some("XenoDialog") && key == "text" {
    // inspired by <https://github.com/L-Sherry/Localize-Me-Tools/blob/07f0b1a4abb9cd553a73dcbdeb3c68eec5f7dcb9/tags.py#L27-L52>
    if let Some(Value::Object(entity)) = object.get("entity") {
      if let (Some(Value::Bool(_entity_global @ true)), Some(Value::String(entity_name))) =
        (entity.get("global"), entity.get("name"))
      {
        if let Some(Value::Array(entities)) = state.file_data.get("entities") {
          //

          if let Some(entity2) = entities.iter().find(|entity2| {
            try_option!({
              let entity2 = entity2.as_object()?;
              let entity2_type = entity2.get("type")?.as_str()?;
              let entity2_name = entity2.get("settings")?.as_object()?.get("name")?.as_str()?;
              entity2_type == "NPC" && entity2_name == entity_name
            })
            .unwrap_or(false)
          }) {
            if let Some(Value::String(character_name)) =
              entity2.get("settings").and_then(|s| s.get("characterName"))
            {
              state.words.push(character_name.into());
            }
          }

          //
        }
      }
    }

    //
  } else if let Some(Value::String(type_)) = object.get("type") {
    // The two common object types to have a string "type" field are event (and
    // action) steps and entities, we are mostly interested in these.
    state.words.push(type_.into());

    if let (Some(Value::Object(settings)), Some(Value::Number(_x)), Some(Value::Number(_y))) =
      (object.get("settings"), object.get("x"), object.get("y"))
    {
      // Looks like this is an entity.
      state.current_entity_type = Some(type_);

      if let Some(Value::String(name)) = settings.get("name") {
        if !name.is_empty() {
          // Not all entities have a name, actually, the most frequent entity
          // types to do so are Prop and ItemDestruct.
          state.words.push(name.into());
        }
      }

      if let Some(Value::String(start_condition)) = settings.get("startCondition") {
        if !start_condition.is_empty() {
          state.words.push("START IF".into());
          state.words.push(start_condition.into());
        }
      }

      if let Some(Value::String(spawn_condition)) = settings.get("spawnCondition") {
        if !spawn_condition.is_empty() {
          state.words.push("SPAWN IF".into());
          state.words.push(spawn_condition.into());
        }
      }

      //
    } else {
      // Most likely an event step.

      #[allow(clippy::single_match)]
      match type_.as_str() {
        "IF" => {
          match key {
            "thenStep" => {}
            "elseStep" => state.words.push("NOT".into()),
            _ => state.words.push(key.into()),
          }
          if let Some(Value::String(condition)) = object.get("condition") {
            if !condition.is_empty() {
              state.words.push(condition.into());
            }
          }
        }

        _ => {
          match object.get("person") {
            Some(Value::String(person)) => {
              state.words.push(person.into());
              state.words.push("@DEFAULT".into());
            }
            Some(Value::Object(person)) => {
              if let (Some(Value::String(person)), Some(Value::String(expression))) =
                (person.get("person"), person.get("expression"))
              {
                state.words.push(person.into());
                state.words.push(utils::fast_concat(&["@", expression]).into());
              }
            }
            _ => {}
          };
        }
      }

      //
    }

    //
  } else if let Some(Value::String(condition)) = object.get("condition") {
    if !condition.is_empty() {
      state.words.push("IF".into());
      state.words.push(condition.into());
    }
  }

  if !state.words.is_empty() {
    let line = state.words.join(" ");
    let line = line.trim();
    if !line.is_empty() {
      state.description.push(RcString::from(line));
    }
  }

  Some(())
}
