//! Parsing command-line strings into exa options.
//!
//! This module imports exa’s configuration types, such as `View` (the details
//! of displaying multiple files) and `DirAction` (what to do when encountering
//! a directory), and implements `deduce` methods on them so they can be
//! configured using command-line options.
//!
//!
//! ## Useless and overridden options
//!
//! Let’s say exa was invoked with just one argument: `exa --inode`. The
//! `--inode` option is used in the details view, where it adds the inode
//! column to the output. But because the details view is *only* activated with
//! the `--long` argument, adding `--inode` without it would not have any
//! effect.
//!
//! For a long time, exa’s philosophy was that the user should be warned
//! whenever they could be mistaken like this. If you tell exa to display the
//! inode, and it *doesn’t* display the inode, isn’t that more annoying than
//! having it throw an error back at you?
//!
//! However, this doesn’t take into account *configuration*. Say a user wants
//! to configure exa so that it lists inodes in the details view, but otherwise
//! functions normally. A common way to do this for command-line programs is to
//! define a shell alias that specifies the details they want to use every
//! time. For the inode column, the alias would be:
//!
//! `alias exa="exa --inode"`
//!
//! Using this alias means that although the inode column will be shown in the
//! details view, you’re now *only* allowed to use the details view, as any
//! other view type will result in an error. Oops!
//!
//! Another example is when an option is specified twice, such as `exa
//! --sort=Name --sort=size`. Did the user change their mind about sorting, and
//! accidentally specify the option twice?
//!
//! Again, exa rejected this case, throwing an error back to the user instead
//! of trying to guess how they want their output sorted. And again, this
//! doesn’t take into account aliases being used to set defaults. A user who
//! wants their files to be sorted case-insensitively may configure their shell
//! with the following:
//!
//! `alias exa="exa --sort=Name"`
//!
//! Just like the earlier example, the user now can’t use any other sort order,
//! because exa refuses to guess which one they meant. It’s *more* annoying to
//! have to go back and edit the command than if there were no error.
//!
//! Fortunately, there’s a heuristic for telling which options came from an
//! alias and which came from the actual command-line: aliased options are
//! nearer the beginning of the options array, and command-line options are
//! nearer the end. This means that after the options have been parsed, exa
//! needs to traverse them *backwards* to find the last-most-specified one.
//!
//! For example, invoking exa with `exa --sort=size` when that alias is present
//! would result in a full command-line of:
//!
//! `exa --sort=Name --sort=size`
//!
//! `--sort=size` should override `--sort=Name` because it’s closer to the end
//! of the arguments array. In fact, because there’s no way to tell where the
//! arguments came from -- it’s just a heuristic -- this will still work even
//! if no aliases are being used!
//!
//! Finally, this isn’t just useful when options could override each other.
//! Creating an alias `exal=”exa --long --inode --header”` then invoking `exal
//! --grid --long` shouldn’t complain about `--long` being given twice when
//! it’s clear what the user wants.


use std::ffi::OsStr;

use getopts;

use fs::feature::xattr;
use fs::dir_action::DirAction;
use fs::filter::FileFilter;
use output::{View, Mode};
use output::details;

mod dir_action;
mod filter;
mod view;

mod help;
use self::help::HelpString;

mod misfire;
pub use self::misfire::Misfire;

mod parser;


/// These **options** represent a parsed, error-checked versions of the
/// user’s command-line options.
#[derive(Debug)]
pub struct Options {

    /// The action to perform when encountering a directory rather than a
    /// regular file.
    pub dir_action: DirAction,

    /// How to sort and filter files before outputting them.
    pub filter: FileFilter,

    /// The type of output to use (lines, grid, or details).
    pub view: View,
}

impl Options {

    // Even though the arguments go in as OsStrings, they come out
    // as Strings. Invalid UTF-8 won’t be parsed, but it won’t make
    // exa core dump either.
    //
    // https://github.com/rust-lang-nursery/getopts/pull/29

    /// Call getopts on the given slice of command-line strings.
    #[allow(unused_results)]
    pub fn getopts<C>(args: C) -> Result<(Options, Vec<String>), Misfire>
    where C: IntoIterator, C::Item: AsRef<OsStr> {
        let mut opts = getopts::Options::new();

        opts.optflag("v", "version",   "show version of exa");
        opts.optflag("?", "help",      "show list of command-line options");

        // Display options
        opts.optflag("1", "oneline",      "display one entry per line");
        opts.optflag("l", "long",         "display extended file metadata in a table");
        opts.optflag("G", "grid",         "display entries as a grid (default)");
        opts.optflag("x", "across",       "sort the grid across, rather than downwards");
        opts.optflag("R", "recurse",      "recurse into directories");
        opts.optflag("T", "tree",         "recurse into directories as a tree");
        opts.optflag("F", "classify",     "display type indicator by file names (one of */=@|)");
        opts.optopt ("",  "color",        "when to use terminal colours", "WHEN");
        opts.optopt ("",  "colour",       "when to use terminal colours", "WHEN");
        opts.optflag("",  "color-scale",  "highlight levels of file sizes distinctly");
        opts.optflag("",  "colour-scale", "highlight levels of file sizes distinctly");

        // Filtering and sorting options
        opts.optflag("",  "group-directories-first", "sort directories before other files");
        opts.optflagmulti("a", "all",    "show hidden and 'dot' files");
        opts.optflag("d", "list-dirs",   "list directories like regular files");
        opts.optopt ("L", "level",       "limit the depth of recursion", "DEPTH");
        opts.optflag("r", "reverse",     "reverse the sert order");
        opts.optopt ("s", "sort",        "which field to sort by", "WORD");
        opts.optopt ("I", "ignore-glob", "ignore files that match these glob patterns", "GLOB1|GLOB2...");

        // Long view options
        opts.optflag("b", "binary",     "list file sizes with binary prefixes");
        opts.optflag("B", "bytes",      "list file sizes in bytes, without prefixes");
        opts.optflag("g", "group",      "list each file's group");
        opts.optflag("h", "header",     "add a header row to each column");
        opts.optflag("H", "links",      "list each file's number of hard links");
        opts.optflag("i", "inode",      "list each file's inode number");
        opts.optflag("m", "modified",   "use the modified timestamp field");
        opts.optflag("S", "blocks",     "list each file's number of file system blocks");
        opts.optopt ("t", "time",       "which timestamp field to show", "WORD");
        opts.optflag("u", "accessed",   "use the accessed timestamp field");
        opts.optflag("U", "created",    "use the created timestamp field");
        opts.optopt ("",  "time-style", "how to format timestamp fields", "STYLE");

        if cfg!(feature="git") {
            opts.optflag("", "git", "list each file's git status");
        }

        if xattr::ENABLED {
            opts.optflag("@", "extended", "list each file's extended attribute keys and sizes");
        }

        let matches = match opts.parse(args) {
            Ok(m)   => m,
            Err(e)  => return Err(Misfire::InvalidOptions(e)),
        };

        if matches.opt_present("help") {
            let help = HelpString {
                only_long: matches.opt_present("long"),
                git: cfg!(feature="git"),
                xattrs: xattr::ENABLED,
            };

            return Err(Misfire::Help(help));
        }
        else if matches.opt_present("version") {
            return Err(Misfire::Version);
        }

        let options = Options::deduce(&matches)?;
        Ok((options, matches.free))
    }

    /// Whether the View specified in this set of options includes a Git
    /// status column. It’s only worth trying to discover a repository if the
    /// results will end up being displayed.
    pub fn should_scan_for_git(&self) -> bool {
        match self.view.mode {
            Mode::Details(details::Options { table: Some(ref table), .. }) |
            Mode::GridDetails(_, details::Options { table: Some(ref table), .. }) => table.should_scan_for_git(),
            _ => false,
        }
    }

    /// Determines the complete set of options based on the given command-line
    /// arguments, after they’ve been parsed.
    fn deduce(matches: &getopts::Matches) -> Result<Options, Misfire> {
        let dir_action = DirAction::deduce(matches)?;
        let filter = FileFilter::deduce(matches)?;
        let view = View::deduce(matches)?;

        Ok(Options { dir_action, view, filter })
    }
}


#[cfg(test)]
mod test {
    use super::{Options, Misfire};
    use fs::DotFilter;
    use fs::filter::{SortField, SortCase};
    use fs::feature::xattr;

    fn is_helpful<T>(misfire: Result<T, Misfire>) -> bool {
        match misfire {
            Err(Misfire::Help(_)) => true,
            _                     => false,
        }
    }

    #[test]
    fn help() {
        let opts = Options::getopts(&[ "--help".to_string() ]);
        assert!(is_helpful(opts))
    }

    #[test]
    fn help_with_file() {
        let opts = Options::getopts(&[ "--help".to_string(), "me".to_string() ]);
        assert!(is_helpful(opts))
    }

    #[test]
    fn files() {
        let args = Options::getopts(&[ "this file".to_string(), "that file".to_string() ]).unwrap().1;
        assert_eq!(args, vec![ "this file".to_string(), "that file".to_string() ])
    }

    #[test]
    fn no_args() {
        let nothing: Vec<String> = Vec::new();
        let args = Options::getopts(&nothing).unwrap().1;
        assert!(args.is_empty());  // Listing the `.` directory is done in main.rs
    }

    #[test]
    fn file_sizes() {
        let opts = Options::getopts(&[ "--long", "--binary", "--bytes" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Conflict("binary", "bytes"))
    }

    #[test]
    fn just_binary() {
        let opts = Options::getopts(&[ "--binary" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("binary", false, "long"))
    }

    #[test]
    fn just_bytes() {
        let opts = Options::getopts(&[ "--bytes" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("bytes", false, "long"))
    }

    #[test]
    fn long_across() {
        let opts = Options::getopts(&[ "--long", "--across" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("across", true, "long"))
    }

    #[test]
    fn oneline_across() {
        let opts = Options::getopts(&[ "--oneline", "--across" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("across", true, "oneline"))
    }

    #[test]
    fn just_header() {
        let opts = Options::getopts(&[ "--header" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("header", false, "long"))
    }

    #[test]
    fn just_group() {
        let opts = Options::getopts(&[ "--group" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("group", false, "long"))
    }

    #[test]
    fn just_inode() {
        let opts = Options::getopts(&[ "--inode" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("inode", false, "long"))
    }

    #[test]
    fn just_links() {
        let opts = Options::getopts(&[ "--links" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("links", false, "long"))
    }

    #[test]
    fn just_blocks() {
        let opts = Options::getopts(&[ "--blocks" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("blocks", false, "long"))
    }

    #[test]
    fn test_sort_size() {
        let opts = Options::getopts(&[ "--sort=size" ]);
        assert_eq!(opts.unwrap().0.filter.sort_field, SortField::Size);
    }

    #[test]
    fn test_sort_name() {
        let opts = Options::getopts(&[ "--sort=name" ]);
        assert_eq!(opts.unwrap().0.filter.sort_field, SortField::Name(SortCase::Sensitive));
    }

    #[test]
    fn test_sort_name_lowercase() {
        let opts = Options::getopts(&[ "--sort=Name" ]);
        assert_eq!(opts.unwrap().0.filter.sort_field, SortField::Name(SortCase::Insensitive));
    }

    #[test]
    #[cfg(feature="git")]
    fn just_git() {
        let opts = Options::getopts(&[ "--git" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("git", false, "long"))
    }

    #[test]
    fn extended_without_long() {
        if xattr::ENABLED {
            let opts = Options::getopts(&[ "--extended" ]);
            assert_eq!(opts.unwrap_err(), Misfire::Useless("extended", false, "long"))
        }
    }

    #[test]
    fn level_without_recurse_or_tree() {
        let opts = Options::getopts(&[ "--level", "69105" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless2("level", "recurse", "tree"))
    }

    #[test]
    fn all_all_with_tree() {
        let opts = Options::getopts(&[ "--all", "--all", "--tree" ]);
        assert_eq!(opts.unwrap_err(), Misfire::Useless("all --all", true, "tree"))
    }

    #[test]
    fn nowt() {
        let nothing: Vec<String> = Vec::new();
        let dots = Options::getopts(&nothing).unwrap().0.filter.dot_filter;
        assert_eq!(dots, DotFilter::JustFiles);
    }

    #[test]
    fn all() {
        let dots = Options::getopts(&[ "--all".to_string() ]).unwrap().0.filter.dot_filter;
        assert_eq!(dots, DotFilter::Dotfiles);
    }

    #[test]
    fn allall() {
        let dots = Options::getopts(&[ "-a".to_string(), "-a".to_string() ]).unwrap().0.filter.dot_filter;
        assert_eq!(dots, DotFilter::DotfilesAndDots);
    }
}
