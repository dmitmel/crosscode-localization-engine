use super::lexer::{CommentType, Lexer, Token, TokenType};
use super::{CharPos, ParsingError};

use std::borrow::Cow;
use std::iter;

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
  lexer: iter::Peekable<Lexer<'src>>,
  stored_token: Option<Token<'src>>,
  done: bool,
  current_token_start_pos: CharPos,
  current_token_end_pos: CharPos,
}

impl<'src> Parser<'src> {
  pub fn new(lexer: Lexer<'src>) -> Self {
    Self {
      lexer: lexer.peekable(),
      stored_token: None,
      done: false,
      current_token_start_pos: CharPos { index: 0, line: 0, column: 0 },
      current_token_end_pos: CharPos { index: 0, line: 0, column: 0 },
    }
  }

  fn next_token(&mut self) -> Result<Option<TokenType<'src>>, ParsingError> {
    match self.lexer.next() {
      Some(Ok(token)) => {
        self.current_token_start_pos = token.start_pos;
        self.current_token_end_pos = token.end_pos;
        Ok(Some(token.type_))
      }
      Some(Err(error)) => {
        self.done = true;
        Err(error)
      }
      None => {
        self.done = true;
        Ok(None)
      }
    }
  }

  fn peek_token(&mut self) -> Result<Option<&TokenType<'src>>, ParsingError> {
    match self.lexer.peek() {
      Some(Ok(token)) => Ok(Some(&token.type_)),
      Some(Err(error)) => {
        self.done = true;
        Err(error.clone())
      }
      None => {
        self.done = true;
        Ok(None)
      }
    }
  }

  fn emit_error(&mut self, message: String) -> Result<(), ParsingError> {
    self.done = true;
    Err(ParsingError { pos: self.current_token_start_pos, message })
  }

  fn emit_error_after(&mut self, message: String) -> Result<(), ParsingError> {
    self.done = true;
    Err(ParsingError { pos: self.current_token_end_pos, message })
  }

  fn parse_next_message(&mut self) -> Result<Option<ParsedMessage<'src>>, ParsingError> {
    if self.done {
      return Ok(None);
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

    self.parse_comments_block(&mut message)?;

    if let Some(TokenType::Msgctxt) = self.peek_token()? {
      self.next_token()?;
      self.parse_string_list(&mut message.msgctxt)?;
    }

    if let Some(TokenType::Msgid) = self.next_token()? {
      self.parse_string_list(&mut message.msgid)?;
    } else {
      self.emit_error("expected msgctxt or msgid".to_owned())?;
    }

    if let Some(TokenType::Msgstr) = self.next_token()? {
      self.parse_string_list(&mut message.msgstr)?;
    } else {
      self.emit_error("expected msgstr".to_owned())?;
    }

    Ok(Some(message))
  }

  fn parse_string_list(&mut self, out: &mut Vec<Cow<'src, str>>) -> Result<(), ParsingError> {
    let mut found_any_strings = false;
    while self.peek_token()?.map_or(false, |t| matches!(t, TokenType::String(..))) {
      let text = match self.next_token()? {
        Some(TokenType::String(text)) => text,
        _ => unreachable!(),
      };
      out.push(text);
      found_any_strings = true;
    }
    if !found_any_strings {
      self.emit_error_after("expected one or more strings".to_owned())?;
    }
    Ok(())
  }

  fn parse_comments_block(&mut self, out: &mut ParsedMessage<'src>) -> Result<(), ParsingError> {
    while self.peek_token()?.map_or(false, |t| matches!(t, TokenType::Comment(..))) {
      let (type_, text) = match self.next_token()? {
        Some(TokenType::Comment(type_, text)) => (type_, text),
        _ => unreachable!(),
      };
      let list = match type_ {
        CommentType::Translator => &mut out.translator_comments,
        CommentType::Automatic => &mut out.automatic_comments,
        CommentType::Reference => &mut out.reference_comments,
        CommentType::Flags => &mut out.flags_comments,
      };
      list.push(text);
    }
    Ok(())
  }

  // fn skip_newlines(&mut self) -> Result<(), ParsingError> {
  //   while self.peek_token()?.map_or(false, |t| matches!(t, TokenType::Newline)) {
  //     self.next_token()?;
  //   }
  //   Ok(())
  // }
}

impl<'src> Iterator for Parser<'src> {
  type Item = Result<ParsedMessage<'src>, ParsingError>;

  fn next(&mut self) -> Option<Self::Item> {
    match self.parse_next_message() {
      Ok(Some(v)) => Some(Ok(v)),
      Ok(None) => None,
      Err(e) => Some(Err(e)),
    }
  }
}
