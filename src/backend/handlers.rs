use super::error::BackendNiceError;
use super::{Backend, FieldsSelection, Id, ListedFragment, Method};
use crate::impl_prelude::*;
use crate::project::{FileType, Fragment, GameFileChunk, Project, TrFile, VirtualGameFile};
use crate::rc_string::{MaybeStaticStr, RcString};
use crate::utils::{RcExt, Timestamp};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ReqGetBackendInfo {}

#[derive(Debug, Serialize)]
pub struct ResGetBackendInfo {
  pub implementation_name: MaybeStaticStr,
  pub implementation_version: MaybeStaticStr,
  pub implementation_nice_version: MaybeStaticStr,
}

impl Method for ReqGetBackendInfo {
  fn name() -> &'static str { "get_backend_info" }
  type Result = ResGetBackendInfo;
  fn handler(_backend: &mut Backend, _params: Self) -> AnyResult<Self::Result> {
    Ok(ResGetBackendInfo {
      implementation_name: Cow::Borrowed(crate::CRATE_NAME),
      implementation_version: Cow::Borrowed(crate::CRATE_VERSION),
      implementation_nice_version: Cow::Borrowed(crate::CRATE_NICE_VERSION),
    })
  }
}

inventory::submit!(ReqGetBackendInfo::declaration());

#[derive(Debug, Deserialize)]
pub struct ReqOpenProject {
  pub dir: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct ResOpenProject {
  pub project_id: Id,
}

impl Method for ReqOpenProject {
  fn name() -> &'static str { "open_project" }
  type Result = ResOpenProject;
  fn handler(backend: &mut Backend, params: Self) -> AnyResult<Self::Result> {
    let project = match Project::open(params.dir.clone())
      .with_context(|| format!("Failed to open project in {:?}", params.dir))
    {
      Ok(v) => v,
      Err(e) => backend_nice_error!("failed to open project", e),
    };
    let project_id = backend.project_id.alloc();
    backend.projects.insert(project_id, project);
    Ok(ResOpenProject { project_id })
  }
}

inventory::submit!(ReqOpenProject::declaration());

#[derive(Debug, Deserialize)]
pub struct ReqCloseProject {
  pub project_id: Id,
}

#[derive(Debug, Serialize)]
pub struct ResCloseProject {}

impl Method for ReqCloseProject {
  fn name() -> &'static str { "close_project" }
  type Result = ResCloseProject;
  fn handler(backend: &mut Backend, params: Self) -> AnyResult<Self::Result> {
    match backend.projects.remove(&params.project_id) {
      Some(_project) => Ok(ResCloseProject {}),
      None => backend_nice_error!("project ID not found"),
    }
  }
}

inventory::submit!(ReqCloseProject::declaration());

#[derive(Debug, Deserialize)]
pub struct ReqGetProjectMeta {
  pub project_id: Id,
}

#[derive(Debug, Serialize)]
pub struct ResGetProjectMeta {
  pub root_dir: PathBuf,
  #[serde(with = "crate::utils::serde::CompactUuidHelper")]
  pub id: Uuid,
  pub creation_timestamp: Timestamp,
  pub modification_timestamp: Timestamp,
  pub game_version: RcString,
  pub original_locale: RcString,
  pub reference_locales: Rc<HashSet<RcString>>,
  pub translation_locale: RcString,
  pub translations_dir: RcString,
  pub splitter: MaybeStaticStr,
}

impl Method for ReqGetProjectMeta {
  fn name() -> &'static str { "get_project_meta" }
  type Result = ResGetProjectMeta;
  fn handler(backend: &mut Backend, params: Self) -> AnyResult<Self::Result> {
    let project = match backend.projects.get(&params.project_id) {
      Some(v) => v,
      None => backend_nice_error!("project ID not found"),
    };
    let meta = project.meta();
    Ok(ResGetProjectMeta {
      root_dir: project.root_dir().to_owned(),
      id: meta.id(),
      creation_timestamp: meta.creation_timestamp(),
      modification_timestamp: meta.modification_timestamp(),
      game_version: meta.game_version().share_rc(),
      original_locale: meta.original_locale().share_rc(),
      reference_locales: meta.reference_locales().share_rc(),
      translation_locale: meta.translation_locale().share_rc(),
      translations_dir: meta.translations_dir().share_rc(),
      splitter: Cow::Borrowed(meta.splitter().id()),
    })
  }
}

inventory::submit!(ReqGetProjectMeta::declaration());

#[derive(Debug, Deserialize)]
pub struct ReqListFiles {
  pub project_id: Id,
  pub file_type: FileType,
}

#[derive(Debug, Serialize)]
pub struct ResListFiles {
  pub paths: Vec<RcString>,
}

impl Method for ReqListFiles {
  fn name() -> &'static str { "list_files" }
  type Result = ResListFiles;
  fn handler(backend: &mut Backend, params: Self) -> AnyResult<Self::Result> {
    let project = match backend.projects.get(&params.project_id) {
      Some(v) => v,
      None => backend_nice_error!("project ID not found"),
    };
    let paths: Vec<RcString> = match params.file_type {
      FileType::TrFile => project.tr_files().keys().cloned().collect(),
      FileType::GameFile => project.virtual_game_files().keys().cloned().collect(),
    };
    Ok(ResListFiles { paths })
  }
}

inventory::submit!(ReqListFiles::declaration());

#[derive(Debug, Deserialize)]
pub struct ReqQueryFragments {
  pub project_id: Id,
  pub from_tr_file: Option<String>,
  pub from_game_file: Option<String>,
  pub slice_start: Option<usize>,
  pub slice_end: Option<usize>,
  pub json_paths: Option<Vec<String>>,
  pub select_fields: Rc<FieldsSelection>,
  pub only_count: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ResQueryFragments {
  pub count: usize,
  pub fragments: Vec<Option<ListedFragment>>,
}

impl Method for ReqQueryFragments {
  fn name() -> &'static str { "query_fragments" }
  type Result = ResQueryFragments;
  fn handler(backend: &mut Backend, params: Self) -> AnyResult<Self::Result> {
    let project = match backend.projects.get(&params.project_id) {
      Some(v) => v,
      None => backend_nice_error!("project ID not found"),
    };

    // The code in this handler is modeled in the style of lazy computations,
    // so that we don't spend time creating irrelevant intermediate containers
    // (such as a map of JSON paths to fragments, when no querying by a JSON
    // path is requested).

    let get_game_file = |path: &str| -> AnyResult<Rc<VirtualGameFile>> {
      let game_file = match project.get_virtual_game_file(path) {
        Some(v) => v,
        None => backend_nice_error!("game file not found"),
      };
      Ok(game_file)
    };

    let get_tr_file = |path: &str| -> AnyResult<Rc<TrFile>> {
      let tr_file = match project.get_tr_file(path) {
        Some(v) => v,
        None => backend_nice_error!("tr file not found"),
      };
      Ok(tr_file)
    };

    let get_tr_file_game_file_chunk =
      |tr_file_path: &str, game_file_path: &str| -> AnyResult<Rc<GameFileChunk>> {
        let tr_file = get_tr_file(tr_file_path)?;
        let game_file_chunk = match tr_file.get_game_file_chunk(game_file_path) {
          Some(v) => v,
          None => backend_nice_error!("game file chunk not found"),
        };
        Ok(game_file_chunk)
      };

    let get_sliceable_fragment_list = || -> AnyResult<Vec<Rc<Fragment>>> {
      match (params.from_tr_file.as_ref(), params.from_game_file.as_ref()) {
        (Some(tr_file_path), None) => {
          let tr_file = get_tr_file(tr_file_path)?;
          let mut total = 0;
          for chunk in tr_file.game_file_chunks().values() {
            total += chunk.fragments().len();
          }
          let mut fragments: Vec<Rc<Fragment>> = Vec::with_capacity(total);
          for chunk in tr_file.game_file_chunks().values() {
            fragments.extend(chunk.fragments().values().cloned());
          }
          Ok(fragments)
        }

        (Some(tr_file_path), Some(game_file_path)) => {
          let game_file_chunk = get_tr_file_game_file_chunk(tr_file_path, game_file_path)?;
          let fragments = game_file_chunk.fragments();
          Ok(fragments.values().cloned().collect())
        }

        (None, Some(game_file_path)) => {
          let game_file = get_game_file(game_file_path)?;
          let fragments = game_file.fragments();
          Ok(fragments.values().cloned().collect())
        }

        (None, None) => {
          let mut total = 0;
          for tr_file in project.tr_files().values() {
            for chunk in tr_file.game_file_chunks().values() {
              total += chunk.fragments().len();
            }
          }
          let mut fragments: Vec<Rc<Fragment>> = Vec::with_capacity(total);
          for tr_file in project.tr_files().values() {
            for chunk in tr_file.game_file_chunks().values() {
              fragments.extend(chunk.fragments().values().cloned());
            }
          }
          Ok(fragments)
        }
      }
    };

    let get_fragments_indexed_by_json_path = || -> AnyResult<IndexMap<RcString, Rc<Fragment>>> {
      match (params.from_tr_file.as_ref(), params.from_game_file.as_ref()) {
        (Some(tr_file_path), Some(game_file_path)) => {
          let game_file_chunk = get_tr_file_game_file_chunk(tr_file_path, game_file_path)?;
          let fragments = game_file_chunk.fragments();
          Ok(fragments.clone())
        }

        (None, Some(game_file_path)) => {
          let game_file = get_game_file(game_file_path)?;
          let fragments = game_file.fragments();
          Ok(fragments.clone())
        }

        _ => {
          backend_nice_error!("can't query fragments by JSON path with requested parameters")
        }
      }
    };

    let only_count = params.only_count.unwrap_or(false);
    let new_listed = |fragment: Cow<Rc<Fragment>>| -> Option<ListedFragment> {
      if !only_count {
        Some(ListedFragment {
          fragment: fragment.into_owned(),
          select_fields: params.select_fields.share_rc(),
        })
      } else {
        None
      }
    };

    if let Some(json_paths) = params.json_paths.as_ref() {
      let all_fragments = get_fragments_indexed_by_json_path()?;
      let mut count = 0;
      let mut listed_fragments = Vec::with_capacity(json_paths.len());
      for path in json_paths {
        listed_fragments.push(if let Some(v) = all_fragments.get(path) {
          count += 1;
          new_listed(Cow::Borrowed(v))
        } else {
          None
        });
      }
      return Ok(ResQueryFragments { count, fragments: listed_fragments });
    }

    let mut all_fragments = get_sliceable_fragment_list()?;
    let (slice_start, slice_end) =
      validate_range(all_fragments.len(), (params.slice_start, params.slice_end))?;

    if only_count {
      // Fast path, for counting all fragments in a file.
      return Ok(ResQueryFragments { count: slice_end - slice_start, fragments: Vec::new() });
    }
    let mut listed_fragments = Vec::with_capacity(slice_end - slice_start);
    // Truncate to avoid having to memmove the elements after `slice_end`.
    all_fragments.truncate(slice_end);
    for f in all_fragments.drain(slice_start..slice_end) {
      listed_fragments.push(new_listed(Cow::Owned(f)));
    }
    all_fragments.truncate(0);
    Ok(ResQueryFragments { count: listed_fragments.len(), fragments: listed_fragments })
  }
}

/// Based on <https://github.com/rust-lang/rust/blob/0c341226ad3780c11b1f29f6da8172b1d653f9ef/library/core/src/slice/index.rs#L514-L548>.
fn validate_range(len: usize, range: (Option<usize>, Option<usize>)) -> AnyResult<(usize, usize)> {
  let (start, end) = range;
  let (start, end) = (start.unwrap_or(0), end.unwrap_or(len));
  if start > end {
    backend_nice_error!("start > end");
  }
  if end > len {
    backend_nice_error!("end > len");
  }
  Ok((start, end))
}

inventory::submit!(ReqQueryFragments::declaration());
