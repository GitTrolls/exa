use std::io::{Write, Result as IOResult};

use term_grid as tg;

use fs::File;
use output::colours::Colours;
use output::file_name::{FileName, LinkStyle, Classify};


#[derive(PartialEq, Debug, Copy, Clone)]
pub struct Options {
    pub across: bool,
    pub console_width: usize,
}

impl Options {
    pub fn direction(&self) -> tg::Direction {
        if self.across { tg::Direction::LeftToRight }
                  else { tg::Direction::TopToBottom }
    }
}


pub struct Render<'a> {
    pub files: Vec<File<'a>>,
    pub colours: &'a Colours,
    pub classify: Classify,
    pub opts: &'a Options,
}

impl<'a> Render<'a> {
    pub fn render<W: Write>(&self, w: &mut W) -> IOResult<()> {
        let mut grid = tg::Grid::new(tg::GridOptions {
            direction:  self.opts.direction(),
            filling:    tg::Filling::Spaces(2),
        });

        grid.reserve(self.files.len());

        for file in self.files.iter() {
            let filename = FileName::new(file, LinkStyle::JustFilenames, self.classify, self.colours).paint();
            let width = filename.width();

            grid.add(tg::Cell {
                contents:  filename.strings().to_string(),
                width:     *width,
            });
        }

        if let Some(display) = grid.fit_into_width(self.opts.console_width) {
            write!(w, "{}", display)
        }
        else {
            // File names too long for a grid - drop down to just listing them!
            for file in self.files.iter() {
                let name_cell = FileName::new(file, LinkStyle::JustFilenames, self.classify, self.colours).paint();
                writeln!(w, "{}", name_cell.strings())?;
            }
            Ok(())
        }
    }
}
