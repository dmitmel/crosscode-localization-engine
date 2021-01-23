use super::lexer::{CommentType, Lexer, Token, TokenType};
use super::{CharPos, ParsingError};

use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParsedMessage<'src> {
  pub translator_comments: Vec<Cow<'src, str>>,
  pub automatic_comments: Vec<Cow<'src, str>>,
  pub reference_comments: Vec<Cow<'src, str>>,
  pub flags_comments: Vec<Cow<'src, str>>,
  pub prev_msgctxt: Vec<Cow<'src, str>>,
  pub prev_msgid: Vec<Cow<'src, str>>,
  pub msgctxt: Vec<Cow<'src, str>>,
  pub msgid: Vec<Cow<'src, str>>,
  pub msgstr: Vec<Cow<'src, str>>,
}

#[derive(Debug, Clone)]
pub struct Parser<'src> {
  lexer: Lexer<'src>,
  stored_token: Option<Token<'src>>,
  state: State,
  done: bool,
  current_token_pos: CharPos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum State {
  CommentBlock,
  PrevMsgctxtSection,
  PrevMsgidSection,
  MsgctxtSection,
  MsgidSection,
  MsgstrSection,
}

impl<'src> Parser<'src> {
  pub fn new(lexer: Lexer<'src>) -> Self {
    Self {
      lexer,
      stored_token: None,
      state: State::CommentBlock,
      done: false,
      current_token_pos: CharPos { index: 0, line: 0, column: 0 },
    }
  }

  fn emit_error(&mut self, message: String) -> ParsingError {
    self.done = true;
    ParsingError { pos: self.current_token_pos, message }
  }
}

impl<'src> Iterator for Parser<'src> {
  type Item = Result<ParsedMessage<'src>, ParsingError>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.done {
      return None;
    }

    macro_rules! emit_error {
      ($($arg:tt)*) => {
        {
          return Some(Err(self.emit_error(format!($($arg)*))));
        }
      }
    }

    let mut message = ParsedMessage {
      translator_comments: Vec::new(),
      automatic_comments: Vec::new(),
      reference_comments: Vec::new(),
      flags_comments: Vec::new(),
      prev_msgctxt: Vec::new(),
      prev_msgid: Vec::new(),
      msgctxt: Vec::new(),
      msgid: Vec::new(),
      msgstr: Vec::new(),
    };
    let mut is_previous = false;

    loop {
      let token = match self.stored_token.take() {
        Some(token) => token,
        None => match self.lexer.next() {
          Some(Ok(token)) => token,
          Some(Err(e)) => {
            self.done = true;
            return Some(Err(e));
          }
          None => {
            self.done = true;
            return Some(Ok(message));
          }
        },
      };
      self.current_token_pos = token.start_pos;

      if matches!(token.type_, TokenType::Comment(..)) && self.state != State::CommentBlock {
        self.stored_token = Some(token);
        self.state = State::CommentBlock;
        return Some(Ok(message));
      }

      match token.type_ {
        TokenType::PreviousMarker => {
          is_previous = true;
        }
        TokenType::Newline => {
          is_previous = false;
        }

        TokenType::Msgctxt => {
          if !is_previous {
            self.state = State::MsgctxtSection;
            message.msgctxt = Vec::new();
          } else {
            self.state = State::PrevMsgctxtSection;
            message.prev_msgctxt = Vec::new();
          }
        }

        TokenType::Msgid => {
          if !is_previous {
            self.state = State::MsgidSection;
            message.msgid = Vec::new();
          } else {
            self.state = State::PrevMsgidSection;
            message.prev_msgid = Vec::new();
          }
        }

        TokenType::Msgstr => {
          if !is_previous {
            self.state = State::MsgstrSection;
            message.msgstr = Vec::new();
          } else {
            emit_error!("\"msgstr\" is not allowed here");
          }
        }

        TokenType::String(text) => {
          let (text_buf, expect_previous) = match self.state {
            State::CommentBlock => emit_error!("strings must follow a section keyword"),
            State::PrevMsgctxtSection => (&mut message.prev_msgctxt, true),
            State::PrevMsgidSection => (&mut message.prev_msgid, true),
            State::MsgctxtSection => (&mut message.msgctxt, false),
            State::MsgidSection => (&mut message.msgid, false),
            State::MsgstrSection => (&mut message.msgstr, false),
          };
          if expect_previous == is_previous {
            text_buf.push(text);
          }
        }

        TokenType::Comment(comment_type, text) => {
          if self.state == State::CommentBlock {
            let text_buf = match comment_type {
              CommentType::Translator => &mut message.translator_comments,
              CommentType::Automatic => &mut message.automatic_comments,
              CommentType::Reference => &mut message.reference_comments,
              CommentType::Flags => &mut message.flags_comments,
            };
            text_buf.push(text);
          } else {
            unreachable!();
          }
        }
      }
    }
  }
}
