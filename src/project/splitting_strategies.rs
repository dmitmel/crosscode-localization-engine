use lazy_static::lazy_static;
use std::borrow::Cow;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SplittingStrategyMode {
  PerFile,
  PerFragment,
}

pub trait SplittingStrategy {
  #[inline(always)]
  fn mode(&self) -> SplittingStrategyMode { SplittingStrategyMode::PerFile }

  fn get_translation_file(&mut self, file_path: &str, json_path: &str) -> Cow<'static, str>;
}

fn split_filename_extension(filename: &str) -> (&str, Option<&str>) {
  if let Some(dot_index) = filename.rfind('.') {
    if dot_index > 0 {
      // Safe because `rfind` is guaranteed to return valid character indices.
      let stem = unsafe { filename.get_unchecked(..dot_index) };
      // Safe because in addition to above, byte length of the string "."
      // (which we have to skip and not include in the extension) encoded in
      // UTF-8 is exactly 1.
      let ext = unsafe { filename.get_unchecked(dot_index + 1..) };
      return (stem, Some(ext));
    }
  }
  (filename, None)
}

#[derive(Debug)]
pub struct SameFileTree;

impl SplittingStrategy for SameFileTree {
  fn get_translation_file(&mut self, file_path: &str, _json_path: &str) -> Cow<'static, str> {
    let (file_path, _) = split_filename_extension(file_path);
    Cow::Owned(file_path.to_owned())
  }
}

#[derive(Debug)]
pub struct NotabenoidChapters;

impl SplittingStrategy for NotabenoidChapters {
  // Rewritten from <https://github.com/CCDirectLink/crosscode-ru/blob/93415096b4f01ed4a7f50a20e642e0c9ae07dade/tool/src/Notabenoid.ts#L418-L459>
  #[allow(clippy::single_match)]
  fn get_translation_file(&mut self, file_path: &str, _json_path: &str) -> Cow<'static, str> {
    let mut file_path_components = file_path.split('/');

    match file_path_components.next() {
      Some("extension") => return "extension".into(),

      Some("data") => match file_path_components.next() {
        Some("lang") => return "lang".into(),
        Some("arena") => return "arena".into(),
        Some("enemies") => return "enemies".into(),
        Some("characters") => return "characters".into(),

        Some("maps") => match file_path_components.next() {
          Some(name1) => match AREAS_WITH_CHAPTERS.get(name1) {
            Some(&chapter) => return chapter.into(),
            _ => {}
          },
          _ => {}
        },

        Some("areas") => match file_path_components.next() {
          Some(name1) => match file_path_components.next() {
            None => match split_filename_extension(name1) {
              (file_name, Some("json")) => match AREAS_WITH_CHAPTERS.get(file_name) {
                Some(&chapter) => return chapter.into(),
                _ => {}
              },
              _ => {}
            },
            _ => {}
          },
          _ => {}
        },

        Some(name1) => match file_path_components.next() {
          None => match split_filename_extension(name1) {
            ("database", Some("json")) => return "database".into(),
            ("item-database", Some("json")) => return "item-database".into(),
            _ => {}
          },
          _ => {}
        },

        _ => {}
      },
      _ => {}
    };

    return "etc".into();

    lazy_static! {
      static ref AREAS_WITH_CHAPTERS: HashSet<&'static str> = hashset![
        "arena",
        "arid-dng",
        "arid",
        "autumn-fall",
        "autumn",
        "bergen-trail",
        "bergen",
        "cargo-ship",
        "cold-dng",
        "dreams",
        "flashback",
        "forest",
        "heat-dng",
        "heat-village",
        "heat",
        "hideout",
        "jungle-city",
        "jungle",
        "rhombus-dng",
        "rhombus-sqr",
        "rookie-harbor",
        "shock-dng",
        "tree-dng",
        "wave-dng",
      ];
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_split_filename_extension() {
    assert_eq!(split_filename_extension(""), ("", None));
    assert_eq!(split_filename_extension("name"), ("name", None));
    assert_eq!(split_filename_extension(".name"), (".name", None));
    assert_eq!(split_filename_extension("name."), ("name", Some("")));
    assert_eq!(split_filename_extension(".name."), (".name", Some("")));
    assert_eq!(split_filename_extension("name.ext"), ("name", Some("ext")));
    assert_eq!(split_filename_extension(".name.ext"), (".name", Some("ext")));
    assert_eq!(split_filename_extension("name.ext."), ("name.ext", Some("")));
    assert_eq!(split_filename_extension(".name.ext."), (".name.ext", Some("")));
    assert_eq!(split_filename_extension("name.ext1.ext2"), ("name.ext1", Some("ext2")));
    assert_eq!(split_filename_extension(".name.ext1.ext2"), (".name.ext1", Some("ext2")));
  }
}
