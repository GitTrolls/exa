use std::path::{Path, PathBuf};

use ansi_term::{ANSIString, ANSIStrings};
use git2;

use colours::Colours;

/// Container of Git statuses for all the files in this folder's Git repository.
pub struct Git {
    statuses: Vec<(PathBuf, git2::Status)>,
}

impl Git {

    /// Discover a Git repository on or above this directory, scanning it for
    /// the files' statuses if one is found.
    pub fn scan(path: &Path) -> Result<Git, git2::Error> {
        let repo = try!(git2::Repository::discover(path));
        let workdir = match repo.workdir() {
            Some(w) => w,
            None => return Ok(Git { statuses: vec![] }),  // bare repo
        };

        let statuses = try!(repo.statuses(None)).iter()
                                                .map(|e| (workdir.join(Path::new(e.path().unwrap())), e.status()))
                                                .collect();

        Ok(Git { statuses: statuses })
    }

    /// Get the status for the file at the given path, if present.
    pub fn status(&self, c: &Colours, path: &Path) -> String {
        let status = self.statuses.iter()
                                  .find(|p| p.0.as_path() == path);
        match status {
            Some(&(_, s)) => ANSIStrings( &[Git::index_status(c, s), Git::working_tree_status(c, s) ]).to_string(),
            None => c.punctuation.paint("--").to_string(),
        }
    }

    /// Get the combined status for all the files whose paths begin with the
    /// path that gets passed in. This is used for getting the status of
    /// directories, which don't really have an 'official' status.
    pub fn dir_status(&self, c: &Colours, dir: &Path) -> String {
        let s = self.statuses.iter()
                             .filter(|p| p.0.starts_with(dir))
                             .fold(git2::Status::empty(), |a, b| a | b.1);

        ANSIStrings( &[Git::index_status(c, s), Git::working_tree_status(c, s)] ).to_string()
    }

    /// The character to display if the file has been modified, but not staged.
    fn working_tree_status(colours: &Colours, status: git2::Status) -> ANSIString<'static> {
        match status {
            s if s.contains(git2::STATUS_WT_NEW) => colours.git.new.paint("A"),
            s if s.contains(git2::STATUS_WT_MODIFIED) => colours.git.modified.paint("M"),
            s if s.contains(git2::STATUS_WT_DELETED) => colours.git.deleted.paint("D"),
            s if s.contains(git2::STATUS_WT_RENAMED) => colours.git.renamed.paint("R"),
            s if s.contains(git2::STATUS_WT_TYPECHANGE) => colours.git.typechange.paint("T"),
            _ => colours.punctuation.paint("-"),
        }
    }

    /// The character to display if the file has been modified, and the change
    /// has been staged.
    fn index_status(colours: &Colours, status: git2::Status) -> ANSIString<'static> {
        match status {
            s if s.contains(git2::STATUS_INDEX_NEW) => colours.git.new.paint("A"),
            s if s.contains(git2::STATUS_INDEX_MODIFIED) => colours.git.modified.paint("M"),
            s if s.contains(git2::STATUS_INDEX_DELETED) => colours.git.deleted.paint("D"),
            s if s.contains(git2::STATUS_INDEX_RENAMED) => colours.git.renamed.paint("R"),
            s if s.contains(git2::STATUS_INDEX_TYPECHANGE) => colours.git.typechange.paint("T"),
            _ => colours.punctuation.paint("-"),
        }
    }
}

