use std::env::var_os;

use output::Colours;
use output::{View, Mode, grid, details};
use output::table::{TimeTypes, Environment, SizeFormat, Options as TableOptions};
use output::file_name::{Classify, FileStyle};
use output::time::TimeFormat;

use options::{flags, Misfire};
use options::parser::MatchedFlags;

use fs::feature::xattr;
use info::filetype::FileExtensions;


impl View {

    /// Determine which view to use and all of that view’s arguments.
    pub fn deduce(matches: &MatchedFlags) -> Result<View, Misfire> {
        let mode = Mode::deduce(matches)?;
        let colours = Colours::deduce(matches)?;
        let style = FileStyle::deduce(matches)?;
        Ok(View { mode, colours, style })
    }
}


impl Mode {

    /// Determine the mode from the command-line arguments.
    pub fn deduce(matches: &MatchedFlags) -> Result<Mode, Misfire> {
        use options::misfire::Misfire::*;

        let long = || {
            if matches.has(&flags::ACROSS)? && !matches.has(&flags::GRID)? {
                Err(Useless(&flags::ACROSS, true, &flags::LONG))
            }
            else if matches.has(&flags::ONE_LINE)? {
                Err(Useless(&flags::ONE_LINE, true, &flags::LONG))
            }
            else {
                Ok(details::Options {
                    table: Some(TableOptions::deduce(matches)?),
                    header: matches.has(&flags::HEADER)?,
                    xattr: xattr::ENABLED && matches.has(&flags::EXTENDED)?,
                })
            }
        };

        let long_options_scan = || {
            for option in &[ &flags::BINARY, &flags::BYTES, &flags::INODE, &flags::LINKS,
                             &flags::HEADER, &flags::BLOCKS, &flags::TIME, &flags::GROUP ] {
                if matches.has(option)? {
                    return Err(Useless(*option, false, &flags::LONG));
                }
            }

            if cfg!(feature="git") && matches.has(&flags::GIT)? {
                Err(Useless(&flags::GIT, false, &flags::LONG))
            }
            else if matches.has(&flags::LEVEL)? && !matches.has(&flags::RECURSE)? && !matches.has(&flags::TREE)? {
                Err(Useless2(&flags::LEVEL, &flags::RECURSE, &flags::TREE))
            }
            else {
                Ok(())
            }
        };

        let other_options_scan = || {
            if let Some(width) = TerminalWidth::deduce()?.width() {
                if matches.has(&flags::ONE_LINE)? {
                    if matches.has(&flags::ACROSS)? {
                        Err(Useless(&flags::ACROSS, true, &flags::ONE_LINE))
                    }
                    else {
                        Ok(Mode::Lines)
                    }
                }
                else if matches.has(&flags::TREE)? {
                    let details = details::Options {
                        table: None,
                        header: false,
                        xattr: xattr::ENABLED && matches.has(&flags::EXTENDED)?,
                    };

                    Ok(Mode::Details(details))
                }
                else {
                    let grid = grid::Options {
                        across: matches.has(&flags::ACROSS)?,
                        console_width: width,
                    };

                    Ok(Mode::Grid(grid))
                }
            }
            else {
                // If the terminal width couldn’t be matched for some reason, such
                // as the program’s stdout being connected to a file, then
                // fallback to the lines view.

                if matches.has(&flags::TREE)? {
                    let details = details::Options {
                        table: None,
                        header: false,
                        xattr: xattr::ENABLED && matches.has(&flags::EXTENDED)?,
                    };

                    Ok(Mode::Details(details))
                }
                else {
                    Ok(Mode::Lines)
                }
            }
        };

        if matches.has(&flags::LONG)? {
            let details = long()?;
            if matches.has(&flags::GRID)? {
                match other_options_scan()? {
                    Mode::Grid(grid)  => return Ok(Mode::GridDetails(grid, details)),
                    others            => return Ok(others),
                };
            }
            else {
                return Ok(Mode::Details(details));
            }
        }

        long_options_scan()?;

        other_options_scan()
    }
}


/// The width of the terminal requested by the user.
#[derive(PartialEq, Debug)]
enum TerminalWidth {

    /// The user requested this specific number of columns.
    Set(usize),

    /// The terminal was found to have this number of columns.
    Terminal(usize),

    /// The user didn’t request any particular terminal width.
    Unset,
}

impl TerminalWidth {

    /// Determine a requested terminal width from the command-line arguments.
    ///
    /// Returns an error if a requested width doesn’t parse to an integer.
    fn deduce() -> Result<TerminalWidth, Misfire> {
        if let Some(columns) = var_os("COLUMNS").and_then(|s| s.into_string().ok()) {
            match columns.parse() {
                Ok(width)  => Ok(TerminalWidth::Set(width)),
                Err(e)     => Err(Misfire::FailedParse(e)),
            }
        }
        else if let Some(width) = *TERM_WIDTH {
            Ok(TerminalWidth::Terminal(width))
        }
        else {
            Ok(TerminalWidth::Unset)
        }
    }

    fn width(&self) -> Option<usize> {
        match *self {
            TerminalWidth::Set(width)       |
            TerminalWidth::Terminal(width)  => Some(width),
            TerminalWidth::Unset            => None,
        }
    }
}


impl TableOptions {
    fn deduce(matches: &MatchedFlags) -> Result<Self, Misfire> {
        Ok(TableOptions {
            env:         Environment::load_all(),
            time_format: TimeFormat::deduce(matches)?,
            size_format: SizeFormat::deduce(matches)?,
            time_types:  TimeTypes::deduce(matches)?,
            inode:  matches.has(&flags::INODE)?,
            links:  matches.has(&flags::LINKS)?,
            blocks: matches.has(&flags::BLOCKS)?,
            group:  matches.has(&flags::GROUP)?,
            git:    cfg!(feature="git") && matches.has(&flags::GIT)?,
        })
    }
}


impl SizeFormat {

    /// Determine which file size to use in the file size column based on
    /// the user’s options.
    ///
    /// The default mode is to use the decimal prefixes, as they are the
    /// most commonly-understood, and don’t involve trying to parse large
    /// strings of digits in your head. Changing the format to anything else
    /// involves the `--binary` or `--bytes` flags, and these conflict with
    /// each other.
    fn deduce(matches: &MatchedFlags) -> Result<SizeFormat, Misfire> {
        let binary = matches.has(&flags::BINARY)?;
        let bytes  = matches.has(&flags::BYTES)?;

        match (binary, bytes) {
            (true,  true )  => Err(Misfire::Conflict(&flags::BINARY, &flags::BYTES)),
            (true,  false)  => Ok(SizeFormat::BinaryBytes),
            (false, true )  => Ok(SizeFormat::JustBytes),
            (false, false)  => Ok(SizeFormat::DecimalBytes),
        }
    }
}


const TIME_STYLES: &[&str] = &["default", "long-iso", "full-iso", "iso"];

impl TimeFormat {

    /// Determine how time should be formatted in timestamp columns.
    fn deduce(matches: &MatchedFlags) -> Result<TimeFormat, Misfire> {
        pub use output::time::{DefaultFormat, ISOFormat};

        let word = match matches.get(&flags::TIME_STYLE)? {
            Some(w) => w,
            None    => return Ok(TimeFormat::DefaultFormat(DefaultFormat::new())),
        };

        if word == "default" {
            Ok(TimeFormat::DefaultFormat(DefaultFormat::new()))
        }
        else if word == "iso" {
            Ok(TimeFormat::ISOFormat(ISOFormat::new()))
        }
        else if word == "long-iso" {
            Ok(TimeFormat::LongISO)
        }
        else if word == "full-iso" {
            Ok(TimeFormat::FullISO)
        }
        else {
            Err(Misfire::bad_argument(&flags::TIME_STYLE, word, TIME_STYLES))
        }
    }
}


static TIMES: &[&str] = &["modified", "accessed", "created"];

impl TimeTypes {

    /// Determine which of a file’s time fields should be displayed for it
    /// based on the user’s options.
    ///
    /// There are two separate ways to pick which fields to show: with a
    /// flag (such as `--modified`) or with a parameter (such as
    /// `--time=modified`). An error is signaled if both ways are used.
    ///
    /// It’s valid to show more than one column by passing in more than one
    /// option, but passing *no* options means that the user just wants to
    /// see the default set.
    fn deduce(matches: &MatchedFlags) -> Result<TimeTypes, Misfire> {
        let possible_word = matches.get(&flags::TIME)?;
        let modified = matches.has(&flags::MODIFIED)?;
        let created  = matches.has(&flags::CREATED)?;
        let accessed = matches.has(&flags::ACCESSED)?;

        if let Some(word) = possible_word {
            if modified {
                Err(Misfire::Useless(&flags::MODIFIED, true, &flags::TIME))
            }
            else if created {
                Err(Misfire::Useless(&flags::CREATED, true, &flags::TIME))
            }
            else if accessed {
                Err(Misfire::Useless(&flags::ACCESSED, true, &flags::TIME))
            }
            else if word == "mod" || word == "modified" {
                Ok(TimeTypes { accessed: false, modified: true,  created: false })
            }
            else if word == "acc" || word == "accessed" {
                Ok(TimeTypes { accessed: true,  modified: false, created: false })
            }
            else if word == "cr" || word == "created" {
                Ok(TimeTypes { accessed: false, modified: false, created: true  })
            }
            else {
                Err(Misfire::bad_argument(&flags::TIME, word, TIMES))
            }
        }
        else if modified || created || accessed {
            Ok(TimeTypes { accessed, modified, created })
        }
        else {
            Ok(TimeTypes::default())
        }
    }
}


/// Under what circumstances we should display coloured, rather than plain,
/// output to the terminal.
///
/// By default, we want to display the colours when stdout can display them.
/// Turning them on when output is going to, say, a pipe, would make programs
/// such as `grep` or `more` not work properly. So the `Automatic` mode does
/// this check and only displays colours when they can be truly appreciated.
#[derive(PartialEq, Debug)]
enum TerminalColours {

    /// Display them even when output isn’t going to a terminal.
    Always,

    /// Display them when output is going to a terminal, but not otherwise.
    Automatic,

    /// Never display them, even when output is going to a terminal.
    Never,
}

impl Default for TerminalColours {
    fn default() -> TerminalColours {
        TerminalColours::Automatic
    }
}

const COLOURS: &[&str] = &["always", "auto", "never"];

impl TerminalColours {

    /// Determine which terminal colour conditions to use.
    fn deduce(matches: &MatchedFlags) -> Result<TerminalColours, Misfire> {

        let word = match matches.get_where(|f| f.matches(&flags::COLOR) || f.matches(&flags::COLOUR))? {
            Some(w) => w,
            None    => return Ok(TerminalColours::default()),
        };

        if word == "always" {
            Ok(TerminalColours::Always)
        }
        else if word == "auto" || word == "automatic" {
            Ok(TerminalColours::Automatic)
        }
        else if word == "never" {
            Ok(TerminalColours::Never)
        }
        else {
            Err(Misfire::bad_argument(&flags::COLOR, word, COLOURS))
        }
    }
}


impl Colours {
    fn deduce(matches: &MatchedFlags) -> Result<Colours, Misfire> {
        use self::TerminalColours::*;

        let tc = TerminalColours::deduce(matches)?;
        if tc == Always || (tc == Automatic && TERM_WIDTH.is_some()) {
            let scale = matches.has(&flags::COLOR_SCALE)? || matches.has(&flags::COLOUR_SCALE)?;
            Ok(Colours::colourful(scale))
        }
        else {
            Ok(Colours::plain())
        }
    }
}



impl FileStyle {
    fn deduce(matches: &MatchedFlags) -> Result<FileStyle, Misfire> {
        let classify = Classify::deduce(matches)?;
        let exts = FileExtensions;
        Ok(FileStyle { classify, exts })
    }
}

impl Classify {
    fn deduce(matches: &MatchedFlags) -> Result<Classify, Misfire> {
        let flagged = matches.has(&flags::CLASSIFY)?;

        Ok(if flagged { Classify::AddFileIndicators }
                 else { Classify::JustFilenames })
    }
}


// Gets, then caches, the width of the terminal that exa is running in.
// This gets used multiple times above, with no real guarantee of order,
// so it’s easier to just cache it the first time it runs.
lazy_static! {
    static ref TERM_WIDTH: Option<usize> = {
        // All of stdin, stdout, and stderr could not be connected to a
        // terminal, but we’re only interested in stdout because it’s
        // where the output goes.
        use term_size::dimensions_stdout;
        dimensions_stdout().map(|t| t.0)
    };
}



#[cfg(test)]
mod test {
    use super::*;
    use std::ffi::OsString;
    use options::flags;
    use options::parser::{Flag, Arg};

    use options::test::parse_for_test;
    use options::test::Strictnesses::*;

    pub fn os(input: &'static str) -> OsString {
        let mut os = OsString::new();
        os.push(input);
        os
    }

    static TEST_ARGS: &[&Arg] = &[ &flags::BINARY, &flags::BYTES,    &flags::TIME_STYLE,
                                   &flags::TIME,   &flags::MODIFIED, &flags::CREATED, &flags::ACCESSED,
                                   &flags::COLOR,  &flags::COLOUR ];

    macro_rules! test {
        ($name:ident: $type:ident <- $inputs:expr; $stricts:expr => $result:expr) => {
            #[test]
            fn $name() {
                for result in parse_for_test($inputs.as_ref(), TEST_ARGS, $stricts, |mf| $type::deduce(mf)) {
                    assert_eq!(result, $result);
                }
            }
        };

        ($name:ident: $type:ident <- $inputs:expr; $stricts:expr => like $pat:pat) => {
            #[test]
            fn $name() {
                for result in parse_for_test($inputs.as_ref(), TEST_ARGS, $stricts, |mf| $type::deduce(mf)) {
                    println!("Testing {:?}", result);
                    match result {
                        $pat => assert!(true),
                        _    => assert!(false),
                    }
                }
            }
        };
    }


    mod size_formats {
        use super::*;

        // Default behaviour
        test!(empty:   SizeFormat <- [];                       Both => Ok(SizeFormat::DecimalBytes));

        // Individual flags
        test!(binary:  SizeFormat <- ["--binary"];             Both => Ok(SizeFormat::BinaryBytes));
        test!(bytes:   SizeFormat <- ["--bytes"];              Both => Ok(SizeFormat::JustBytes));

        // Errors
        test!(both:    SizeFormat <- ["--binary", "--bytes"];  Both => Err(Misfire::Conflict(&flags::BINARY, &flags::BYTES)));
    }


    mod time_formats {
        use super::*;

        // These tests use pattern matching because TimeFormat doesn’t
        // implement PartialEq.

        // Default behaviour
        test!(empty:     TimeFormat <- [];                            Both => like Ok(TimeFormat::DefaultFormat(_)));

        // Individual settings
        test!(default:   TimeFormat <- ["--time-style=default"];      Both => like Ok(TimeFormat::DefaultFormat(_)));
        test!(iso:       TimeFormat <- ["--time-style", "iso"];       Both => like Ok(TimeFormat::ISOFormat(_)));
        test!(long_iso:  TimeFormat <- ["--time-style=long-iso"];     Both => like Ok(TimeFormat::LongISO));
        test!(full_iso:  TimeFormat <- ["--time-style", "full-iso"];  Both => like Ok(TimeFormat::FullISO));

        // Overriding
        test!(actually:  TimeFormat <- ["--time-style=default",     "--time-style", "iso"];    Last => like Ok(TimeFormat::ISOFormat(_)));
        test!(actual_2:  TimeFormat <- ["--time-style=default",     "--time-style", "iso"];    Complain => like Err(Misfire::Duplicate(Flag::Long("time-style"), Flag::Long("time-style"))));

        test!(nevermind: TimeFormat <- ["--time-style", "long-iso", "--time-style=full-iso"];  Last => like Ok(TimeFormat::FullISO));
        test!(nevermore: TimeFormat <- ["--time-style", "long-iso", "--time-style=full-iso"];  Complain => like Err(Misfire::Duplicate(Flag::Long("time-style"), Flag::Long("time-style"))));

        // Errors
        test!(daily:     TimeFormat <- ["--time-style=24-hour"];      Both => like Err(Misfire::BadArgument(_, _, _)));
    }


    mod time_types {
        use super::*;

        // Default behaviour
        test!(empty:     TimeTypes <- [];                      Both => Ok(TimeTypes::default()));

        // Modified
        test!(modified:  TimeTypes <- ["--modified"];          Both => Ok(TimeTypes { accessed: false,  modified: true,   created: false }));
        test!(m:         TimeTypes <- ["-m"];                  Both => Ok(TimeTypes { accessed: false,  modified: true,   created: false }));
        test!(time_mod:  TimeTypes <- ["--time=modified"];     Both => Ok(TimeTypes { accessed: false,  modified: true,   created: false }));
        test!(time_m:    TimeTypes <- ["-tmod"];               Both => Ok(TimeTypes { accessed: false,  modified: true,   created: false }));

        // Accessed
        test!(acc:       TimeTypes <- ["--accessed"];          Both => Ok(TimeTypes { accessed: true,   modified: false,  created: false }));
        test!(a:         TimeTypes <- ["-u"];                  Both => Ok(TimeTypes { accessed: true,   modified: false,  created: false }));
        test!(time_acc:  TimeTypes <- ["--time", "accessed"];  Both => Ok(TimeTypes { accessed: true,   modified: false,  created: false }));
        test!(time_a:    TimeTypes <- ["-t", "acc"];           Both => Ok(TimeTypes { accessed: true,   modified: false,  created: false }));

        // Created
        test!(cr:        TimeTypes <- ["--created"];           Both => Ok(TimeTypes { accessed: false,  modified: false,  created: true  }));
        test!(c:         TimeTypes <- ["-U"];                  Both => Ok(TimeTypes { accessed: false,  modified: false,  created: true  }));
        test!(time_cr:   TimeTypes <- ["--time=created"];      Both => Ok(TimeTypes { accessed: false,  modified: false,  created: true  }));
        test!(time_c:    TimeTypes <- ["-tcr"];                Both => Ok(TimeTypes { accessed: false,  modified: false,  created: true  }));

        // Multiples
        test!(time_uu:   TimeTypes <- ["-uU"];                 Both => Ok(TimeTypes { accessed: true,   modified: false,  created: true  }));

        // Errors
        test!(time_tea:  TimeTypes <- ["--time=tea"];          Both => Err(Misfire::bad_argument(&flags::TIME, &os("tea"), super::TIMES)));
        test!(time_ea:   TimeTypes <- ["-tea"];                Both => Err(Misfire::bad_argument(&flags::TIME, &os("ea"), super::TIMES)));

        // Overriding
        test!(overridden:   TimeTypes <- ["-tcr", "-tmod"];    Last => Ok(TimeTypes { accessed: false,  modified: true,   created: false }));
        test!(overridden_2: TimeTypes <- ["-tcr", "-tmod"];    Complain => Err(Misfire::Duplicate(Flag::Short(b't'), Flag::Short(b't'))));
    }


    mod colourses {
        use super::*;

        // Default
        test!(empty:        TerminalColours <- [];                     Both => Ok(TerminalColours::default()));

        // --colour
        test!(u_always:     TerminalColours <- ["--colour=always"];    Both => Ok(TerminalColours::Always));
        test!(u_auto:       TerminalColours <- ["--colour", "auto"];   Both => Ok(TerminalColours::Automatic));
        test!(u_never:      TerminalColours <- ["--colour=never"];     Both => Ok(TerminalColours::Never));

        // --color
        test!(no_u_always:  TerminalColours <- ["--color", "always"];  Both => Ok(TerminalColours::Always));
        test!(no_u_auto:    TerminalColours <- ["--color=auto"];       Both => Ok(TerminalColours::Automatic));
        test!(no_u_never:   TerminalColours <- ["--color", "never"];   Both => Ok(TerminalColours::Never));

        // Errors
        test!(no_u_error:   TerminalColours <- ["--color=upstream"];   Both => Err(Misfire::bad_argument(&flags::COLOR, &os("upstream"), super::COLOURS)));  // the error is for --color
        test!(u_error:      TerminalColours <- ["--colour=lovers"];    Both => Err(Misfire::bad_argument(&flags::COLOR, &os("lovers"),   super::COLOURS)));  // and so is this one!

        // Overriding
        test!(overridden_1: TerminalColours <- ["--colour=auto", "--colour=never"];  Last => Ok(TerminalColours::Never));
        test!(overridden_2: TerminalColours <- ["--color=auto",  "--colour=never"];  Last => Ok(TerminalColours::Never));
        test!(overridden_3: TerminalColours <- ["--colour=auto", "--color=never"];   Last => Ok(TerminalColours::Never));
        test!(overridden_4: TerminalColours <- ["--color=auto",  "--color=never"];   Last => Ok(TerminalColours::Never));

        test!(overridden_5: TerminalColours <- ["--colour=auto", "--colour=never"];  Complain => Err(Misfire::Duplicate(Flag::Long("colour"), Flag::Long("colour"))));
        test!(overridden_6: TerminalColours <- ["--color=auto",  "--colour=never"];  Complain => Err(Misfire::Duplicate(Flag::Long("color"),  Flag::Long("colour"))));
        test!(overridden_7: TerminalColours <- ["--colour=auto", "--color=never"];   Complain => Err(Misfire::Duplicate(Flag::Long("colour"), Flag::Long("color"))));
        test!(overridden_8: TerminalColours <- ["--color=auto",  "--color=never"];   Complain => Err(Misfire::Duplicate(Flag::Long("color"),  Flag::Long("color"))));
    }
}
