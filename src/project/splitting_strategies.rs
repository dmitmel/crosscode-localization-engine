use lazy_static::lazy_static;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

pub trait SplittingStrategy {
  fn get_translation_file_for_entire_game_file(
    &mut self,
    file_path: &str,
  ) -> Option<Cow<'static, str>>;

  fn get_translation_file_for_fragment(
    &mut self,
    file_path: &str,
    _json_path: &str,
  ) -> Cow<'static, str> {
    self.get_translation_file_for_entire_game_file(file_path).unwrap()
  }
}

lazy_static! {
  static ref STRATEGIES_MAP: HashMap<&'static str, fn() -> Box<dyn SplittingStrategy>> = {
    // Don't ask me why the compiler requires the following type annotation
    let mut m: HashMap<_, fn() -> _> = HashMap::new();
    m.insert("monolithic-file", MonolithicFileStrategy::new_box);
    m.insert("same-file-tree", SameFileTreeStrategy::new_box);
    m.insert("notabenoid-chapters", NotabenoidChaptersStrategy::new_box);
    m
  };
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
pub struct MonolithicFileStrategy;

impl MonolithicFileStrategy {
  fn new_box() -> Box<dyn SplittingStrategy> { Box::new(Self) }
}

impl SplittingStrategy for MonolithicFileStrategy {
  fn get_translation_file_for_entire_game_file(
    &mut self,
    _file_path: &str,
  ) -> Option<Cow<'static, str>> {
    Some("translation".into())
  }
}

#[derive(Debug)]
pub struct SameFileTreeStrategy;

impl SameFileTreeStrategy {
  fn new_box() -> Box<dyn SplittingStrategy> { Box::new(Self) }
}

impl SplittingStrategy for SameFileTreeStrategy {
  fn get_translation_file_for_entire_game_file(
    &mut self,
    file_path: &str,
  ) -> Option<Cow<'static, str>> {
    let (file_path, _) = split_filename_extension(file_path);
    Some(file_path.to_owned().into())
  }
}

#[derive(Debug)]
pub struct NotabenoidChaptersStrategy;

impl NotabenoidChaptersStrategy {
  fn new_box() -> Box<dyn SplittingStrategy> { Box::new(Self) }
}

impl SplittingStrategy for NotabenoidChaptersStrategy {
  // Rewritten from <https://github.com/CCDirectLink/crosscode-ru/blob/93415096b4f01ed4a7f50a20e642e0c9ae07dade/tool/src/Notabenoid.ts#L418-L459>
  #[allow(clippy::single_match)]
  fn get_translation_file_for_entire_game_file(
    &mut self,
    file_path: &str,
  ) -> Option<Cow<'static, str>> {
    return Some(inner(file_path).into());

    fn inner(file_path: &str) -> &'static str {
      let components: Vec<_> = file_path.split('/').collect();
      if !components.is_empty() {
        match components[0] {
          "extension" => return "extension",
          "data" if components.len() > 1 => match components[1] {
            "lang" => return "lang",
            "arena" => return "arena",
            "enemies" => return "enemies",
            "characters" => return "characters",

            "maps" if components.len() > 2 => match AREAS_WITH_CHAPTERS.get(components[2]) {
              Some(chapter) => return chapter,
              _ => {}
            },

            "areas" if components.len() == 3 => match split_filename_extension(components[2]) {
              (area_name, Some("json")) => match AREAS_WITH_CHAPTERS.get(area_name) {
                Some(&chapter) => return chapter,
                _ => {}
              },
              _ => {}
            },

            _ if components.len() == 2 => match components[1] {
              "database.json" => return "database",
              "item-database.json" => return "item-database",
              _ => {}
            },

            _ => {}
          },
          _ => {}
        }
      }

      return "etc";

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
