use output::Colours;

use options::{flags, Misfire};
use options::parser::MatchedFlags;



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
    pub fn deduce<TW>(matches: &MatchedFlags, widther: TW) -> Result<Colours, Misfire>
    where TW: Fn() -> Option<usize> {
        use self::TerminalColours::*;

        let tc = TerminalColours::deduce(matches)?;
        if tc == Always || (tc == Automatic && widther().is_some()) {
            let scale = matches.has_where(|f| f.matches(&flags::COLOR_SCALE) || f.matches(&flags::COLOUR_SCALE))?;
            Ok(Colours::colourful(scale.is_some()))
        }
        else {
            Ok(Colours::plain())
        }
    }
}


#[cfg(test)]
mod terminal_test {
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

    static TEST_ARGS: &[&Arg] = &[ &flags::COLOR, &flags::COLOUR ];

    macro_rules! test {
        ($name:ident:  $inputs:expr;  $stricts:expr => $result:expr) => {
            #[test]
            fn $name() {
                for result in parse_for_test($inputs.as_ref(), TEST_ARGS, $stricts, |mf| TerminalColours::deduce(mf)) {
                    assert_eq!(result, $result);
                }
            }
        };

        ($name:ident:  $inputs:expr;  $stricts:expr => err $result:expr) => {
            #[test]
            fn $name() {
                for result in parse_for_test($inputs.as_ref(), TEST_ARGS, $stricts, |mf| TerminalColours::deduce(mf)) {
                    assert_eq!(result.unwrap_err(), $result);
                }
            }
        };
    }


    // Default
    test!(empty:         [];                     Both => Ok(TerminalColours::default()));

    // --colour
    test!(u_always:      ["--colour=always"];    Both => Ok(TerminalColours::Always));
    test!(u_auto:        ["--colour", "auto"];   Both => Ok(TerminalColours::Automatic));
    test!(u_never:       ["--colour=never"];     Both => Ok(TerminalColours::Never));

    // --color
    test!(no_u_always:   ["--color", "always"];  Both => Ok(TerminalColours::Always));
    test!(no_u_auto:     ["--color=auto"];       Both => Ok(TerminalColours::Automatic));
    test!(no_u_never:    ["--color", "never"];   Both => Ok(TerminalColours::Never));

    // Errors
    test!(no_u_error:    ["--color=upstream"];   Both => err Misfire::bad_argument(&flags::COLOR, &os("upstream"), super::COLOURS));  // the error is for --color
    test!(u_error:       ["--colour=lovers"];    Both => err Misfire::bad_argument(&flags::COLOR, &os("lovers"),   super::COLOURS));  // and so is this one!

    // Overriding
    test!(overridden_1:  ["--colour=auto", "--colour=never"];  Last => Ok(TerminalColours::Never));
    test!(overridden_2:  ["--color=auto",  "--colour=never"];  Last => Ok(TerminalColours::Never));
    test!(overridden_3:  ["--colour=auto", "--color=never"];   Last => Ok(TerminalColours::Never));
    test!(overridden_4:  ["--color=auto",  "--color=never"];   Last => Ok(TerminalColours::Never));

    test!(overridden_5:  ["--colour=auto", "--colour=never"];  Complain => err Misfire::Duplicate(Flag::Long("colour"), Flag::Long("colour")));
    test!(overridden_6:  ["--color=auto",  "--colour=never"];  Complain => err Misfire::Duplicate(Flag::Long("color"),  Flag::Long("colour")));
    test!(overridden_7:  ["--colour=auto", "--color=never"];   Complain => err Misfire::Duplicate(Flag::Long("colour"), Flag::Long("color")));
    test!(overridden_8:  ["--color=auto",  "--color=never"];   Complain => err Misfire::Duplicate(Flag::Long("color"),  Flag::Long("color")));
}


#[cfg(test)]
mod colour_test {
    use super::*;
    use options::flags;
    use options::parser::{Flag, Arg};

    use options::test::parse_for_test;
    use options::test::Strictnesses::*;

    static TEST_ARGS: &[&Arg] = &[ &flags::COLOR,       &flags::COLOUR,
                                   &flags::COLOR_SCALE, &flags::COLOUR_SCALE ];

    macro_rules! test {
        ($name:ident:  $inputs:expr, $widther:expr;  $stricts:expr => $result:expr) => {
            #[test]
            fn $name() {
                for result in parse_for_test($inputs.as_ref(), TEST_ARGS, $stricts, |mf| Colours::deduce(mf, &$widther)) {
                    assert_eq!(result, $result);
                }
            }
        };

        ($name:ident:  $inputs:expr, $widther:expr;  $stricts:expr => err $result:expr) => {
            #[test]
            fn $name() {
                for result in parse_for_test($inputs.as_ref(), TEST_ARGS, $stricts, |mf| Colours::deduce(mf, &$widther)) {
                    assert_eq!(result.unwrap_err(), $result);
                }
            }
        };

        ($name:ident:  $inputs:expr, $widther:expr;  $stricts:expr => like $pat:pat) => {
            #[test]
            fn $name() {
                for result in parse_for_test($inputs.as_ref(), TEST_ARGS, $stricts, |mf| Colours::deduce(mf, &$widther)) {
                    println!("Testing {:?}", result);
                    match result {
                        $pat => assert!(true),
                        _    => assert!(false),
                    }
                }
            }
        };
    }

    test!(width_1:  ["--colour", "always"],    || Some(80);  Both => Ok(Colours::colourful(false)));
    test!(width_2:  ["--colour", "always"],    || None;      Both => Ok(Colours::colourful(false)));
    test!(width_3:  ["--colour", "never"],     || Some(80);  Both => Ok(Colours::plain()));
    test!(width_4:  ["--colour", "never"],     || None;      Both => Ok(Colours::plain()));
    test!(width_5:  ["--colour", "automatic"], || Some(80);  Both => Ok(Colours::colourful(false)));
    test!(width_6:  ["--colour", "automatic"], || None;      Both => Ok(Colours::plain()));
    test!(width_7:  [],                        || Some(80);  Both => Ok(Colours::colourful(false)));
    test!(width_8:  [],                        || None;      Both => Ok(Colours::plain()));

    test!(scale_1:  ["--color=always", "--color-scale", "--colour-scale"], || None;   Last => like Ok(Colours { scale: true,  .. }));
    test!(scale_2:  ["--color=always", "--color-scale",                 ], || None;   Last => like Ok(Colours { scale: true,  .. }));
    test!(scale_3:  ["--color=always",                  "--colour-scale"], || None;   Last => like Ok(Colours { scale: true,  .. }));
    test!(scale_4:  ["--color=always",                                  ], || None;   Last => like Ok(Colours { scale: false, .. }));

    test!(scale_5:  ["--color=always", "--color-scale", "--colour-scale"], || None;   Complain => err Misfire::Duplicate(Flag::Long("color-scale"),  Flag::Long("colour-scale")));
    test!(scale_6:  ["--color=always", "--color-scale",                 ], || None;   Complain => like Ok(Colours { scale: true,  .. }));
    test!(scale_7:  ["--color=always",                  "--colour-scale"], || None;   Complain => like Ok(Colours { scale: true,  .. }));
    test!(scale_8:  ["--color=always",                                  ], || None;   Complain => like Ok(Colours { scale: false, .. }));
}
