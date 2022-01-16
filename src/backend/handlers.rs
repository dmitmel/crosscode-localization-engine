use super::error::BackendNiceError;
use super::{Backend, FieldsSelection, Id, ListedFragment, Method};
use crate::impl_prelude::*;
use crate::project::Project;
use crate::rc_string::{MaybeStaticStr, RcString};
use crate::utils::{RcExt, Timestamp};

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ReqBackendInfo {}

#[derive(Debug, Serialize)]
pub struct ResBackendInfo {
  pub implementation_name: MaybeStaticStr,
  pub implementation_version: MaybeStaticStr,
  pub implementation_nice_version: MaybeStaticStr,
}

impl Method for ReqBackendInfo {
  fn name() -> &'static str { "Backend/info" }
  type Result = ResBackendInfo;
  fn handler(_backend: &mut Backend, _params: Self) -> AnyResult<Self::Result> {
    Ok(ResBackendInfo {
      implementation_name: Cow::Borrowed(crate::CRATE_NAME),
      implementation_version: Cow::Borrowed(crate::CRATE_VERSION),
      implementation_nice_version: Cow::Borrowed(crate::CRATE_NICE_VERSION),
    })
  }
}

inventory::submit!(ReqBackendInfo::declaration());

#[derive(Debug, Deserialize)]
pub struct ReqProjectOpen {
  pub dir: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct ResProjectOpen {
  pub project_id: Id,
}

impl Method for ReqProjectOpen {
  fn name() -> &'static str { "Project/open" }
  type Result = ResProjectOpen;
  fn handler(backend: &mut Backend, params: Self) -> AnyResult<Self::Result> {
    let project = match Project::open(params.dir.clone())
      .with_context(|| format!("Failed to open project in {:?}", params.dir))
    {
      Ok(v) => v,
      Err(e) => backend_nice_error!("failed to open project", e),
    };
    let project_id = backend.project_id_alloc.next().unwrap();
    backend.projects.insert(project_id, project);
    Ok(ResProjectOpen { project_id })
  }
}

inventory::submit!(ReqProjectOpen::declaration());

#[derive(Debug, Deserialize)]
pub struct ReqProjectClose {
  pub project_id: Id,
}

#[derive(Debug, Serialize)]
pub struct ResProjectClose {}

impl Method for ReqProjectClose {
  fn name() -> &'static str { "Project/close" }
  type Result = ResProjectClose;
  fn handler(backend: &mut Backend, params: Self) -> AnyResult<Self::Result> {
    match backend.projects.remove(&params.project_id) {
      Some(_project) => Ok(ResProjectClose {}),
      None => backend_nice_error!("project ID not found"),
    }
  }
}

inventory::submit!(ReqProjectClose::declaration());

#[derive(Debug, Deserialize)]
pub struct ReqProjectGetMeta {
  pub project_id: Id,
}

#[derive(Debug, Serialize)]
pub struct ResProjectGetMeta {
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

impl Method for ReqProjectGetMeta {
  fn name() -> &'static str { "Project/get_meta" }
  type Result = ResProjectGetMeta;
  fn handler(backend: &mut Backend, params: Self) -> AnyResult<Self::Result> {
    let project = match backend.projects.get(&params.project_id) {
      Some(v) => v,
      None => backend_nice_error!("project ID not found"),
    };
    let meta = project.meta();
    Ok(ResProjectGetMeta {
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

inventory::submit!(ReqProjectGetMeta::declaration());

#[derive(Debug, Deserialize)]
pub struct ReqProjectListTrFiles {
  pub project_id: Id,
}

#[derive(Debug, Serialize)]
pub struct ResProjectListTrFiles {
  pub paths: Vec<RcString>,
}

impl Method for ReqProjectListTrFiles {
  fn name() -> &'static str { "Project/list_tr_files" }
  type Result = ResProjectListTrFiles;
  fn handler(backend: &mut Backend, params: Self) -> AnyResult<Self::Result> {
    let project = match backend.projects.get(&params.project_id) {
      Some(v) => v,
      None => backend_nice_error!("project ID not found"),
    };
    let paths: Vec<RcString> = project.tr_files().keys().cloned().collect();
    Ok(ResProjectListTrFiles { paths })
  }
}

inventory::submit!(ReqProjectListTrFiles::declaration());

#[derive(Debug, Deserialize)]
pub struct ReqProjectListVirtualGameFiles {
  pub project_id: Id,
}

#[derive(Debug, Serialize)]
pub struct ResProjectListVirtualGameFiles {
  pub paths: Vec<RcString>,
}

impl Method for ReqProjectListVirtualGameFiles {
  fn name() -> &'static str { "Project/list_virtual_game_files" }
  type Result = ResProjectListVirtualGameFiles;
  fn handler(backend: &mut Backend, params: Self) -> AnyResult<Self::Result> {
    let project = match backend.projects.get(&params.project_id) {
      Some(v) => v,
      None => backend_nice_error!("project ID not found"),
    };
    let paths: Vec<RcString> = project.virtual_game_files().keys().cloned().collect();
    Ok(ResProjectListVirtualGameFiles { paths })
  }
}

inventory::submit!(ReqProjectListVirtualGameFiles::declaration());

#[derive(Debug, Deserialize)]
pub struct ReqVirtualGameFileListFragments {
  pub project_id: Id,
  pub file_path: String,
  pub start: Option<usize>,
  pub end: Option<usize>,
  pub select_fields: Rc<FieldsSelection>,
}

#[derive(Debug, Serialize)]
pub struct ResVirtualGameFileListFragments {
  pub fragments: Vec<ListedFragment>,
}

impl Method for ReqVirtualGameFileListFragments {
  fn name() -> &'static str { "VirtualGameFile/list_fragments" }
  type Result = ResVirtualGameFileListFragments;
  fn handler(backend: &mut Backend, params: Self) -> AnyResult<Self::Result> {
    let project = match backend.projects.get(&params.project_id) {
      Some(v) => v,
      None => backend_nice_error!("project ID not found"),
    };
    let game_file = match project.get_virtual_game_file(&params.file_path) {
      Some(v) => v,
      None => backend_nice_error!("virtual game file not found"),
    };
    let all_fragments = game_file.fragments();
    let (start, end) = validate_range(all_fragments.len(), (params.start, params.end))?;
    let mut listed_fragments = Vec::with_capacity(end.checked_sub(start).unwrap());

    for i in start..end {
      let (_, f) = all_fragments.get_index(i).unwrap();
      listed_fragments.push(ListedFragment {
        fragment: f.share_rc(),
        select_fields: params.select_fields.share_rc(),
      });
    }

    Ok(ResVirtualGameFileListFragments { fragments: listed_fragments })
  }
}

inventory::submit!(ReqVirtualGameFileListFragments::declaration());

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

#[derive(Debug, Deserialize)]
pub struct ReqVirtualGameFileGetFragment {
  pub project_id: Id,
  pub file_path: String,
  pub json_path: String,
  pub select_fields: Rc<FieldsSelection>,
}

#[derive(Debug, Serialize)]
pub struct ResVirtualGameFileGetFragment {
  pub fragment: ListedFragment,
}

impl Method for ReqVirtualGameFileGetFragment {
  fn name() -> &'static str { "VirtualGameFile/get_fragment" }
  type Result = ResVirtualGameFileGetFragment;
  fn handler(backend: &mut Backend, params: Self) -> AnyResult<Self::Result> {
    let project = match backend.projects.get(&params.project_id) {
      Some(v) => v,
      None => backend_nice_error!("project ID not found"),
    };
    let game_file = match project.get_virtual_game_file(&params.file_path) {
      Some(v) => v,
      None => backend_nice_error!("virtual game file not found"),
    };
    let f = match game_file.get_fragment(&params.json_path) {
      Some(v) => v,
      None => backend_nice_error!("virtual game file not found"),
    };
    Ok(ResVirtualGameFileGetFragment {
      fragment: ListedFragment { fragment: f.share_rc(), select_fields: params.select_fields },
    })
  }
}

inventory::submit!(ReqVirtualGameFileGetFragment::declaration());
