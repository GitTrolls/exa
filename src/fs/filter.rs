use std::cmp::Ordering;
use std::iter::FromIterator;
use std::os::unix::fs::MetadataExt;

use glob;
use natord;

use fs::File;
use fs::DotFilter;


/// The **file filter** processes a vector of files before outputting them,
/// filtering and sorting the files depending on the user’s command-line
/// flags.
#[derive(PartialEq, Debug, Clone)]
pub struct FileFilter {

    /// Whether directories should be listed first, and other types of file
    /// second. Some users prefer it like this.
    pub list_dirs_first: bool,

    /// The metadata field to sort by.
    pub sort_field: SortField,

    /// Whether to reverse the sorting order. This would sort the largest
    /// files first, or files starting with Z, or the most-recently-changed
    /// ones, depending on the sort field.
    pub reverse: bool,

    /// Which invisible “dot” files to include when listing a directory.
    ///
    /// Files starting with a single “.” are used to determine “system” or
    /// “configuration” files that should not be displayed in a regular
    /// directory listing, and the directory entries “.” and “..” are
    /// considered extra-special.
    ///
    /// This came about more or less by a complete historical accident,
    /// when the original `ls` tried to hide `.` and `..`:
    /// https://plus.google.com/+RobPikeTheHuman/posts/R58WgWwN9jp
    ///
    ///   When one typed ls, however, these files appeared, so either Ken or
    ///   Dennis added a simple test to the program. It was in assembler then,
    ///   but the code in question was equivalent to something like this:
    ///      if (name[0] == '.') continue;
    ///   This statement was a little shorter than what it should have been,
    ///   which is:
    ///      if (strcmp(name, ".") == 0 || strcmp(name, "..") == 0) continue;
    ///   but hey, it was easy.
    ///
    ///   Two things resulted.
    ///
    ///   First, a bad precedent was set. A lot of other lazy programmers
    ///   introduced bugs by making the same simplification. Actual files
    ///   beginning with periods are often skipped when they should be counted.
    ///
    ///   Second, and much worse, the idea of a "hidden" or "dot" file was
    ///   created. As a consequence, more lazy programmers started dropping
    ///   files into everyone's home directory. I don't have all that much
    ///   stuff installed on the machine I'm using to type this, but my home
    ///   directory has about a hundred dot files and I don't even know what
    ///   most of them are or whether they're still needed. Every file name
    ///   evaluation that goes through my home directory is slowed down by
    ///   this accumulated sludge.
    pub dot_filter: DotFilter,

    /// Glob patterns to ignore. Any file name that matches *any* of these
    /// patterns won't be displayed in the list.
    pub ignore_patterns: IgnorePatterns,
}


impl FileFilter {
/// Remove every file in the given vector that does *not* pass the
   /// filter predicate for files found inside a directory.
   pub fn filter_child_files(&self, files: &mut Vec<File>) {
       files.retain(|f| !self.ignore_patterns.is_ignored(f));
   }

   /// Remove every file in the given vector that does *not* pass the
   /// filter predicate for file names specified on the command-line.
   ///
   /// The rules are different for these types of files than the other
   /// type because the ignore rules can be used with globbing. For
   /// example, running "exa -I='*.tmp' .vimrc" shouldn't filter out the
   /// dotfile, because it's been directly specified. But running
   /// "exa -I='*.ogg' music/*" should filter out the ogg files obtained
   /// from the glob, even though the globbing is done by the shell!
   pub fn filter_argument_files(&self, files: &mut Vec<File>) {
       files.retain(|f| !self.ignore_patterns.is_ignored(f));
   }

   /// Sort the files in the given vector based on the sort field option.
   pub fn sort_files<'a, F>(&self, files: &mut Vec<F>)
   where F: AsRef<File<'a>> {

       files.sort_by(|a, b| self.compare_files(a.as_ref(), b.as_ref()));

       if self.reverse {
           files.reverse();
       }

       if self.list_dirs_first {
           // This relies on the fact that `sort_by` is stable.
           files.sort_by(|a, b| b.as_ref().is_directory().cmp(&a.as_ref().is_directory()));
       }
   }

   /// Compares two files to determine the order they should be listed in,
   /// depending on the search field.
   pub fn compare_files(&self, a: &File, b: &File) -> Ordering {
       use self::SortCase::{Sensitive, Insensitive};

       match self.sort_field {
           SortField::Unsorted  => Ordering::Equal,

           SortField::Name(Sensitive)    => natord::compare(&a.name, &b.name),
           SortField::Name(Insensitive)  => natord::compare_ignore_case(&a.name, &b.name),

           SortField::Size          => a.metadata.len().cmp(&b.metadata.len()),
           SortField::FileInode     => a.metadata.ino().cmp(&b.metadata.ino()),
           SortField::ModifiedDate  => a.metadata.mtime().cmp(&b.metadata.mtime()),
           SortField::AccessedDate  => a.metadata.atime().cmp(&b.metadata.atime()),
           SortField::CreatedDate   => a.metadata.ctime().cmp(&b.metadata.ctime()),

           SortField::FileType => match a.type_char().cmp(&b.type_char()) { // todo: this recomputes
               Ordering::Equal  => natord::compare(&*a.name, &*b.name),
               order            => order,
           },

           SortField::Extension(Sensitive) => match a.ext.cmp(&b.ext) {
               Ordering::Equal  => natord::compare(&*a.name, &*b.name),
               order            => order,
           },

           SortField::Extension(Insensitive) => match a.ext.cmp(&b.ext) {
               Ordering::Equal  => natord::compare_ignore_case(&*a.name, &*b.name),
               order            => order,
           },
       }
   }
}


/// User-supplied field to sort by.
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum SortField {

    /// Don't apply any sorting. This is usually used as an optimisation in
    /// scripts, where the order doesn't matter.
    Unsorted,

    /// The file name. This is the default sorting.
    Name(SortCase),

    /// The file's extension, with extensionless files being listed first.
    Extension(SortCase),

    /// The file's size.
    Size,

    /// The file's inode. This is sometimes analogous to the order in which
    /// the files were created on the hard drive.
    FileInode,

    /// The time at which this file was modified (the `mtime`).
    ///
    /// As this is stored as a Unix timestamp, rather than a local time
    /// instance, the time zone does not matter and will only be used to
    /// display the timestamps, not compare them.
    ModifiedDate,

    /// The time at this file was accessed (the `atime`).
    ///
    /// Oddly enough, this field rarely holds the *actual* accessed time.
    /// Recording a read time means writing to the file each time it’s read
    /// slows the whole operation down, so many systems will only update the
    /// timestamp in certain circumstances. This has become common enough that
    /// it’s now expected behaviour for the `atime` field.
    /// http://unix.stackexchange.com/a/8842
    AccessedDate,

    /// The time at which this file was changed or created (the `ctime`).
    ///
    /// Contrary to the name, this field is used to mark the time when a
    /// file's metadata changed -- its permissions, owners, or link count.
    ///
    /// In original Unix, this was, however, meant as creation time.
    /// https://www.bell-labs.com/usr/dmr/www/cacm.html
    CreatedDate,

    /// The type of the file: directories, links, pipes, regular, files, etc.
    ///
    /// Files are ordered according to the `PartialOrd` implementation of
    /// `fs::fields::Type`, so changing that will change this.
    FileType,
}

/// Whether a field should be sorted case-sensitively or case-insensitively.
///
/// This determines which of the `natord` functions to use.
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum SortCase {

    /// Sort files case-sensitively with uppercase first, with ‘A’ coming
    /// before ‘a’.
    Sensitive,

    /// Sort files case-insensitively, with ‘A’ being equal to ‘a’.
    Insensitive,
}


/// The **ignore patterns** are a list of globs that are tested against
/// each filename, and if any of them match, that file isn’t displayed.
/// This lets a user hide, say, text files by ignoring `*.txt`.
#[derive(PartialEq, Default, Debug, Clone)]
pub struct IgnorePatterns {
    patterns: Vec<glob::Pattern>,
}

impl FromIterator<glob::Pattern> for IgnorePatterns {
    fn from_iter<I: IntoIterator<Item = glob::Pattern>>(iter: I) -> Self {
        IgnorePatterns { patterns: iter.into_iter().collect() }
    }
}

impl IgnorePatterns {

    /// Create a new list from the input glob strings, turning the inputs that
    /// are valid glob patterns into an IgnorePatterns. The inputs that don’t
    /// parse correctly are returned separately.
    pub fn parse_from_iter<'a, I: IntoIterator<Item = &'a str>>(iter: I) -> (Self, Vec<glob::PatternError>) {
        let mut patterns = Vec::new();
        let mut errors = Vec::new();

        for input in iter {
            match glob::Pattern::new(input) {
                Ok(pat) => patterns.push(pat),
                Err(e)  => errors.push(e),
            }
        }

        (IgnorePatterns { patterns }, errors)
    }

    /// Create a new empty list that matches nothing.
    pub fn empty() -> IgnorePatterns {
        IgnorePatterns { patterns: Vec::new() }
    }

    /// Test whether the given file should be hidden from the results.
    fn is_ignored(&self, file: &File) -> bool {
        self.patterns.iter().any(|p| p.matches(&file.name))
    }
}
