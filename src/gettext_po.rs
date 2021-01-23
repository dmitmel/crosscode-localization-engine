// See:
// <https://github.com/autotools-mirror/gettext/blob/6c9cff1221f2cbf585fbee6f86ff047c8ede5286/gettext-tools/src/po-gram-gen.y>
// <https://github.com/autotools-mirror/gettext/blob/6c9cff1221f2cbf585fbee6f86ff047c8ede5286/gettext-tools/src/po-lex.c>
// <https://www.gnu.org/software/gettext/manual/gettext.html#PO-Files>
// <https://www.gnu.org/software/gettext/manual/gettext.html#Filling-in-the-Header-Entry>
// <https://www.gnu.org/software/gettext/manual/gettext.html#Invoking-the-msginit-Program>
// <https://github.com/izimobil/polib/blob/0ab9af63d227d30fb261c2dd496ee74f91844a86/polib.py>
// <https://github.com/translate/translate/blob/88d13bea244b1894a4bedf67ba5b8b65cc29d3b0/translate/storage/pypo.py>
// <https://github.com/translate/translate/blob/88d13bea244b1894a4bedf67ba5b8b65cc29d3b0/translate/storage/cpo.py>
// <https://docs.oasis-open.org/xliff/v1.2/xliff-profile-po-1.2-pr-02-20061016-DIFF.pdf>
//
// For testing the behavior of GNU gettext the following Python program can be
// used (launch with the environment variable `USECPO` set to `1`):
//
//     from translate.storage.po import pofile
//     import sys
//     POFile(sys.stdin.buffer).serialize(sys.stdout.buffer)
//
// Needless to say, it requires installation of <https://github.com/translate/translate>
// (also see <https://github.com/translate/translate/blob/88d13bea244b1894a4bedf67ba5b8b65cc29d3b0/translate/storage/po.py>).

pub mod lexer;
pub mod parser;

pub use lexer::Lexer;
pub use parser::{ParsedMessage, Parser};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CharPos {
  pub index: usize,
  pub line: usize,
  pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParsingError {
  pub pos: CharPos,
  pub message: String,
}

pub fn parse(src: &str) -> Parser { Parser::new(Lexer::new(src)) }
