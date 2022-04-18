use crate::localize_me;
use crate::rc_string::MaybeStaticStr;
use crate::utils;

use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::fmt;

pub type SplitterDeclaration = utils::StrategyDeclaration<(), Box<dyn Splitter>>;

assert_trait_is_object_safe!(Splitter);
pub trait Splitter: fmt::Debug + Send + Sync {
  fn id_static() -> &'static str
  where
    Self: Sized;

  fn new_boxed() -> Box<dyn Splitter>
  where
    Self: Sized;

  fn id(&self) -> &'static str;

  fn get_tr_file_for_entire_game_file(
    &mut self,
    asset_root: &str,
    file_path: &str,
  ) -> Option<MaybeStaticStr>;

  fn get_tr_file_for_fragment(
    &mut self,
    asset_root: &str,
    file_path: &str,
    _json_path: &str,
  ) -> MaybeStaticStr {
    self.get_tr_file_for_entire_game_file(asset_root, file_path).unwrap()
  }

  fn declaration() -> SplitterDeclaration
  where
    Self: Sized,
  {
    SplitterDeclaration { id: Self::id_static(), ctor: |_| Ok(Self::new_boxed()) }
  }
}

pub static REGISTRY: Lazy<utils::StrategicalRegistry<(), Box<dyn Splitter>>> = Lazy::new(|| {
  utils::StrategicalRegistry::new(&[
    MonolithicFileSplitter::declaration(),
    SameFileTreeSplitter::declaration(),
    LocalizeMeFileTreeSplitter::declaration(),
    NotabenoidChaptersSplitter::declaration(),
    NextGenerationSplitter::declaration(),
  ])
});

impl<'de> serde::Deserialize<'de> for Box<dyn Splitter> {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let id = <&str>::deserialize(deserializer)?;
    REGISTRY.create(id, ()).map_err(|_| serde::de::Error::unknown_variant(id, REGISTRY.ids()))
  }
}

impl serde::Serialize for Box<dyn Splitter> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    serializer.serialize_str(self.id())
  }
}

#[derive(Debug)]
pub struct MonolithicFileSplitter;

impl MonolithicFileSplitter {
  pub const ID: &'static str = "monolithic-file";
}

impl Splitter for MonolithicFileSplitter {
  #[inline(always)]
  fn id_static() -> &'static str
  where
    Self: Sized,
  {
    MonolithicFileSplitter::ID
  }

  #[inline(always)]
  fn new_boxed() -> Box<dyn Splitter>
  where
    Self: Sized,
  {
    Box::new(Self)
  }

  #[inline(always)]
  fn id(&self) -> &'static str { Self::ID }

  fn get_tr_file_for_entire_game_file(
    &mut self,
    _asset_root: &str,
    _file_path: &str,
  ) -> Option<MaybeStaticStr> {
    Some("translation".into())
  }
}

#[derive(Debug)]
pub struct SameFileTreeSplitter;

impl SameFileTreeSplitter {
  pub const ID: &'static str = "same-file-tree";
}

impl Splitter for SameFileTreeSplitter {
  #[inline(always)]
  fn id_static() -> &'static str
  where
    Self: Sized,
  {
    Self::ID
  }

  #[inline(always)]
  fn new_boxed() -> Box<dyn Splitter>
  where
    Self: Sized,
  {
    Box::new(Self)
  }

  #[inline(always)]
  fn id(&self) -> &'static str { Self::ID }

  fn get_tr_file_for_entire_game_file(
    &mut self,
    _asset_root: &str,
    file_path: &str,
  ) -> Option<MaybeStaticStr> {
    let (file_path, _) = utils::split_filename_extension(file_path);
    Some(file_path.to_owned().into())
  }
}

#[derive(Debug)]
pub struct LocalizeMeFileTreeSplitter;

impl LocalizeMeFileTreeSplitter {
  pub const ID: &'static str = "lm-file-tree";
}

impl Splitter for LocalizeMeFileTreeSplitter {
  #[inline(always)]
  fn id_static() -> &'static str
  where
    Self: Sized,
  {
    Self::ID
  }

  #[inline(always)]
  fn new_boxed() -> Box<dyn Splitter>
  where
    Self: Sized,
  {
    Box::new(Self)
  }

  #[inline(always)]
  fn id(&self) -> &'static str { Self::ID }

  fn get_tr_file_for_entire_game_file(
    &mut self,
    _asset_root: &str,
    file_path: &str,
  ) -> Option<MaybeStaticStr> {
    let file_path = localize_me::serialize_file_path(file_path);
    let (file_path, _) = utils::split_filename_extension(file_path);
    Some(file_path.to_owned().into())
  }
}

#[derive(Debug)]
pub struct NotabenoidChaptersSplitter;

impl NotabenoidChaptersSplitter {
  pub const ID: &'static str = "notabenoid-chapters";
}

impl Splitter for NotabenoidChaptersSplitter {
  #[inline(always)]
  fn id_static() -> &'static str
  where
    Self: Sized,
  {
    Self::ID
  }

  #[inline(always)]
  fn new_boxed() -> Box<dyn Splitter>
  where
    Self: Sized,
  {
    Box::new(Self)
  }

  #[inline(always)]
  fn id(&self) -> &'static str { Self::ID }

  // Rewritten from <https://github.com/CCDirectLink/crosscode-ru/blob/93415096b4f01ed4a7f50a20e642e0c9ae07dade/tool/src/Notabenoid.ts#L418-L459>
  #[allow(clippy::single_match)]
  fn get_tr_file_for_entire_game_file(
    &mut self,
    asset_root: &str,
    file_path: &str,
  ) -> Option<MaybeStaticStr> {
    return Some(inner(asset_root, file_path).into());

    fn inner(asset_root: &str, file_path: &str) -> &'static str {
      let components: Vec<_> = file_path.strip_prefix(asset_root).unwrap().split('/').collect();
      match components[0] {
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

      static AREAS_WITH_CHAPTERS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
        hashset![
          "arena",
          "arid",
          "arid-dng",
          "autumn",
          "autumn-fall",
          "beach",
          "bergen",
          "bergen-trail",
          "cargo-ship",
          "cold-dng",
          "dreams",
          "evo-village",
          "final-dng",
          "flashback",
          "forest",
          "heat",
          "heat-dng",
          "heat-village",
          "hideout",
          "jungle",
          "jungle-city",
          "rhombus-dng",
          "rhombus-sqr",
          "rookie-harbor",
          "shock-dng",
          "tree-dng",
          "wave-dng",
        ]
      });
    }
  }
}

#[derive(Debug)]
pub struct NextGenerationSplitter;

impl NextGenerationSplitter {
  pub const ID: &'static str = "next-generation";
}

impl Splitter for NextGenerationSplitter {
  #[inline(always)]
  fn id_static() -> &'static str
  where
    Self: Sized,
  {
    Self::ID
  }

  #[inline(always)]
  fn new_boxed() -> Box<dyn Splitter>
  where
    Self: Sized,
  {
    Box::new(Self)
  }

  #[inline(always)]
  fn id(&self) -> &'static str { Self::ID }

  #[allow(clippy::single_match)]
  fn get_tr_file_for_entire_game_file(
    &mut self,
    asset_root: &str,
    file_path: &str,
  ) -> Option<MaybeStaticStr> {
    let full_components: Vec<_> = file_path.split('/').collect();
    match *full_components.as_slice() {
      ["extension", extension_dir_name, maybe_extension_manifest] => {
        if maybe_extension_manifest
          .strip_suffix(".json")
          .map_or(false, |file_name| file_name == extension_dir_name)
        {
          return Some("extensions".into());
        }
      }
      _ => {}
    }

    let components: Vec<_> = file_path.strip_prefix(asset_root).unwrap().split('/').collect();

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

    Some(SameFileTreeSplitter.get_tr_file_for_entire_game_file(asset_root, file_path).unwrap())
  }

  fn get_tr_file_for_fragment(
    &mut self,
    asset_root: &str,
    file_path: &str,
    json_path: &str,
  ) -> MaybeStaticStr {
    let components: Vec<_> = file_path.strip_prefix(asset_root).unwrap().split('/').collect();
    let json_components: Vec<_> = json_path.split('/').collect();
    let tr_file_path =
      SameFileTreeSplitter.get_tr_file_for_entire_game_file(asset_root, file_path).unwrap();

    match components[0] {
      "data" if components.len() > 1 => match components[1] {
        "database.json" if components.len() == 2 => {
          return utils::fast_concat(&[&tr_file_path, "/", match json_components[0] {
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
