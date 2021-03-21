//! Port of <https://github.com/dmitmel/cc-translateinator/blob/a36da6700fe028cfbe8e19f89110774e50989fe5/src/crosscode_markup.ts>,
//! which in turn was inspired by <https://github.com/L-Sherry/Localize-Me-Tools/blob/c117847bc15fe8b62a7bcd7f343310c9a4ce09da/checker.py#L118-L165>.

use crate::utils::parsing::{self, ParsingError};

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fmt::{self, Write as _};

pub static FONT_COLORS: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
  hashmap![
    ("0", "#ffffff"), // white (default)
    ("1", "#ff6969"), // red
    ("2", "#65ff89"), // green
    ("3", "#ffe430"), // yellow
    ("4", "#808080"), // gray
    ("5", "#ff8932"), // orange (only on the small font)
  ]
});

pub fn lex(src: &str) -> Lexer { Lexer::new(src) }

pub fn to_string<'src>(tokens: impl Iterator<Item = &'src Token<'src>>, out: &mut String) {
  for token in tokens {
    write!(out, "{}", token).unwrap();
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'src> {
  pub start_pos: usize,
  pub end_pos: usize,
  pub type_: TokenType,
  pub data: &'src str,
}

impl fmt::Display for Token<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    use TokenType::*;
    match self.type_ {
      LiteralText => write!(f, "{}", self.data),
      TypingDelay => write!(f, "\\{}", self.data),
      Escape => write!(f, "\\{}", self.data),
      Color => write!(f, "\\c[{}]", self.data),
      TypingSpeed => write!(f, "\\s[{}]", self.data),
      Variable => write!(f, "\\v[{}]", self.data),
      Icon => write!(f, "\\i[{}]", self.data),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenType {
  LiteralText,
  TypingDelay,
  Escape,
  Color,
  TypingSpeed,
  Variable,
  Icon,
}

#[derive(Debug)]
pub struct Lexer<'src> {
  src: &'src str,
  current_pos: usize,
}

impl<'src> Lexer<'src> {
  pub fn new(src: &'src str) -> Self { Self { src, current_pos: 0 } }

  // fn emit_error(&mut self, message: String) -> Result<!, ParsingError> {
  //   Err(ParsingError {
  //     pos: CharPos { byte_index: self.current_pos, char_index: 0, line: 0, column: 0 },
  //     message: RcString::from(message),
  //   })
  // }

  pub fn parse_next_token(&mut self) -> Result<Option<Token<'src>>, ParsingError> {
    if self.current_pos >= self.src.len() {
      return Ok(None);
    }

    let src = &self.src[self.current_pos..];
    let token: Token = match src.find('\\') {
      Some(0) => {
        let mut i = 1;
        let mut token_type: Option<TokenType> = None;
        let mut token_data = "";

        if let Some(command_char) = src.get(i..i + 1) {
          i += 1;
          token_data = command_char;

          match command_char {
            "." | "!" => token_type = Some(TokenType::TypingDelay),
            "\\" => token_type = Some(TokenType::Escape),

            "c" | "s" | "v" | "i" => {
              if src.get(i..i + 1) == Some("[") {
                i += 1;
                if let Some(j) = parsing::find_start_at(src, i, ']') {
                  token_data = &src[i..j];
                  i = j + 1;
                  match command_char {
                    "c" => token_type = Some(TokenType::Color),
                    "s" => token_type = Some(TokenType::TypingSpeed),
                    "v" => token_type = Some(TokenType::Variable),
                    "i" => token_type = Some(TokenType::Icon),
                    _ => {}
                  };
                }
              }
            }

            _ => {}
          }
        };

        if token_type.is_none() {
          token_data = &src[..i];
        }
        Token {
          start_pos: self.current_pos,
          end_pos: self.current_pos + i,
          type_: token_type.unwrap_or(TokenType::LiteralText),
          data: token_data,
        }
      }

      Some(backslash_index) => Token {
        start_pos: self.current_pos,
        end_pos: self.current_pos + backslash_index,
        type_: TokenType::LiteralText,
        data: &src[..backslash_index],
      },

      None if !src.is_empty() => Token {
        start_pos: self.current_pos,
        end_pos: self.src.len(),
        type_: TokenType::LiteralText,
        data: src,
      },

      None => {
        self.current_pos = self.src.len();
        return Ok(None);
      }
    };

    self.current_pos = token.end_pos;
    Ok(Some(token))
  }
}

impl<'src> Iterator for Lexer<'src> {
  type Item = Result<Token<'src>, ParsingError>;

  fn next(&mut self) -> Option<Self::Item> {
    match self.parse_next_token() {
      Ok(Some(v)) => Some(Ok(v)),
      Ok(None) => None,
      Err(e) => Some(Err(e)),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::TokenType::*;
  use super::*;

  type TestResult<'src> = Result<Vec<(TokenType, &'src str)>, ParsingError>;

  pub fn test_lex(src: &str) -> TestResult {
    Lexer::new(src)
      .map(|t| {
        let t = t?;
        Ok((t.type_, t.data))
      })
      .collect()
  }

  #[test]
  fn test_basic_roundtrip_with_unicode_chars() {
    let text = "\
      \n\\s[1]CrossCode разрабатывался с учётом \\c[3]вызова для игрока\\c[0], как в \
      \\c[3]сражениях\\c[0], так и в \\c[3]головоломках\\c[0], и мы призываем всех игроков \
      попробовать игру на предустановленной сложности.\n\nОднако, если это делает игру слишком \
      сложной или даже непроходимой для вас, в меню \\c[3]настроек\\c[0] имеется \
      \\c[3]вкладка\\c[0] c детальными настройками сложности.";

    let mut roundtrip_text = String::with_capacity(text.len());
    let tokens = lex(text).collect::<Result<Vec<_>, ParsingError>>().unwrap();
    to_string(tokens.iter(), &mut roundtrip_text);
    assert_eq!(roundtrip_text, text);
  }

  #[test]
  fn test_basic_with_unicode_chars() {
    let text = "\
      \n\\s[1]CrossCode разрабатывался с учётом \\c[3]вызова для игрока\\c[0], как в \
      \\c[3]сражениях\\c[0], так и в \\c[3]головоломках\\c[0], и мы призываем всех игроков \
      попробовать игру на предустановленной сложности.\n\nОднако, если это делает игру слишком \
      сложной или даже непроходимой для вас, в меню \\c[3]настроек\\c[0] имеется \
      \\c[3]вкладка\\c[0] c детальными настройками сложности.";

    let tokens: TestResult = Ok(vec![
      (LiteralText, "\n"),
      (TypingSpeed, "1"),
      (LiteralText, "CrossCode разрабатывался с учётом "),
      (Color, "3"),
      (LiteralText, "вызова для игрока"),
      (Color, "0"),
      (LiteralText, ", как в "),
      (Color, "3"),
      (LiteralText, "сражениях"),
      (Color, "0"),
      (LiteralText, ", так и в "),
      (Color, "3"),
      (LiteralText, "головоломках"),
      (Color, "0"),
      (
        LiteralText,
        ", и мы призываем всех игроков попробовать игру на предустановленной сложности.\n\n\
        Однако, если это делает игру слишком сложной или даже непроходимой для вас, в меню ",
      ),
      (Color, "3"),
      (LiteralText, "настроек"),
      (Color, "0"),
      (LiteralText, " имеется "),
      (Color, "3"),
      (LiteralText, "вкладка"),
      (Color, "0"),
      (LiteralText, " c детальными настройками сложности."),
    ]);

    assert_eq!(test_lex(text), tokens);
  }
}
