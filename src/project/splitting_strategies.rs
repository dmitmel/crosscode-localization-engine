use crate::utils;

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
  pub static ref SPLITTING_STRATEGIES_MAP: HashMap<&'static str, fn() -> Box<dyn SplittingStrategy>> = {
    macro_rules! strategies_map {
      ($($strat:ident,)+) => { strategies_map![$($strat),+] };
      ($($strat:ident),*) => {
        {
          let _cap = count_exprs!($($strat),*);
          // Don't ask me why the compiler requires the following type
          // annotation.
          let mut _map: HashMap<_, fn() -> _> = HashMap::with_capacity(_cap);
          $(let _ = _map.insert($strat::ID, $strat::new_box);)*
          _map
        }
      };
    }
    strategies_map![
      MonolithicFileStrategy,
      SameFileTreeStrategy,
      NotabenoidChaptersStrategy,
      NextGenerationStrategy,
    ]
  };
}

#[derive(Debug)]
pub struct MonolithicFileStrategy;

impl MonolithicFileStrategy {
  pub const ID: &'static str = "monolithic-file";
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
  pub const ID: &'static str = "same-file-tree";
  fn new_box() -> Box<dyn SplittingStrategy> { Box::new(Self) }
}

impl SplittingStrategy for SameFileTreeStrategy {
  fn get_translation_file_for_entire_game_file(
    &mut self,
    file_path: &str,
  ) -> Option<Cow<'static, str>> {
    let (file_path, _) = utils::split_filename_extension(file_path);
    Some(file_path.to_owned().into())
  }
}

#[derive(Debug)]
pub struct NotabenoidChaptersStrategy;

impl NotabenoidChaptersStrategy {
  pub const ID: &'static str = "notabenoid-chapters";
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

          "areas" if components.len() == 3 => match utils::split_filename_extension(components[2])
          {
            (area_name, Some("json")) => match AREAS_WITH_CHAPTERS.get(area_name) {
              Some(&chapter) => return chapter,
              _ => {}
            },
            _ => {}
          },

          "database.json" if components.len() == 2 => return "database",
          "item-database.json" if components.len() == 2 => return "item-database",

          _ => {}
        },
        _ => {}
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

#[derive(Debug)]
pub struct NextGenerationStrategy {
  same_file_tree: SameFileTreeStrategy,
}

impl NextGenerationStrategy {
  pub const ID: &'static str = "next-generation";
  fn new_box() -> Box<dyn SplittingStrategy> {
    Box::new(Self { same_file_tree: SameFileTreeStrategy })
  }
}

#[allow(clippy::single_match)]
impl SplittingStrategy for NextGenerationStrategy {
  fn get_translation_file_for_entire_game_file(
    &mut self,
    file_path: &str,
  ) -> Option<Cow<'static, str>> {
    let components: Vec<_> = file_path.split('/').collect();

    match components[0] {
      "data" => match components[1] {
        "areas" => return Some("data/areas".into()),
        "arena" => return Some("data/arena".into()),
        "characters" => return Some("data/characters".into()),
        "credits" => return Some("data/credits".into()),
        "events" => return Some("data/events".into()),
        "lang" => return Some("data/lang".into()),
        "players" => return Some("data/players".into()),
        "save-presets" => return Some("data/save-presets".into()),

        "enemies" => return Some("data/enemies".into()),

        "database.json" if components.len() == 2 => return None,

        _ => {}
      },
      _ => {}
    }

    self.same_file_tree.get_translation_file_for_entire_game_file(file_path)
  }

  fn get_translation_file_for_fragment(
    &mut self,
    file_path: &str,
    json_path: &str,
  ) -> Cow<'static, str> {
    let components: Vec<_> = file_path.split('/').collect();
    let json_components: Vec<_> = json_path.split('/').collect();

    match components[0] {
      "data" if components.len() > 1 => match components[1] {
        "database.json" if components.len() == 2 => {
          return utils::fast_concat(&[file_path, "/", match json_components[0] {
            // "achievements" => "achievements",
            "commonEvents" => "commonEvents",
            "enemies" => "enemies",
            "lore" => "lore",
            "quests" => "quests",
            _ => "other",
          }])
          .into();
        }
        _ => {}
      },
      _ => {}
    }

    unreachable!()
  }
}
