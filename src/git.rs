use std::{
	ffi::OsString,
	path::Path,
	process::{Command, Stdio},
};

use ansi_to_tui::IntoText;
use git2::{Diff, DiffStatsFormat, Oid, Repository, Revwalk};
use tui::text::Text;

pub struct CommitInfo<'repo> {
	pub commit_id: Oid,
	pub author_name: String,
	pub author_email: String,
	pub time: String,
	pub summary: String,
	pub message: String,
	pub patch: Diff<'repo>,
	pub stats: Vec<String>,
}

pub fn log(repo: &Repository, commit_id: Oid) -> Result<Revwalk, git2::Error> {
	let mut revwalk = repo.revwalk()?;
	revwalk.push(commit_id)?;
	return Ok(revwalk);
}

pub fn next_commit<'repo>(
	repo: &'repo Repository,
	revwalk: &mut Revwalk,
) -> Result<Option<CommitInfo<'repo>>, git2::Error> {
	let commit_id = match revwalk.next() {
		None => return Ok(None),
		Some(result) => result?,
	};
	let commit = repo.find_commit(commit_id)?;
	let author = commit.author();
	let time = match chrono::DateTime::from_timestamp(commit.time().seconds(), 0) {
		Some(dt) => format!("{}", dt.with_timezone(&chrono::Local).format("%c")),
		None => "".to_string(),
	};

	let tree: git2::Tree;
	let parent_tree = match commit.parent(0) {
		Ok(parent) => {
			tree = parent.tree()?;
			Some(&tree)
		},
		Err(_) => None,
	};
	let patch = repo.diff_tree_to_tree(parent_tree, Some(&commit.tree()?), None)?;
	let stats = patch
		.stats()?
		.to_buf(DiffStatsFormat::FULL | DiffStatsFormat::INCLUDE_SUMMARY, 100)?
		.as_str()
		.unwrap_or_default()
		.lines()
		.map(|line| line.to_owned())
		.collect();

	return Ok(Some(CommitInfo {
		commit_id,
		author_name: author.name().unwrap_or_default().to_owned(),
		author_email: author.email().unwrap_or_default().to_owned(),
		time,
		summary: commit.summary().unwrap_or_default().to_owned(),
		message: commit.message().unwrap_or_default().to_owned(),
		patch,
		stats,
	}));
}

pub fn show(repo: &Repository, commit_id: Oid, file_path: &Path) -> Text<'static> {
	let repo_path = repo.workdir().unwrap();
	let git_show = match Command::new("git")
		.args([
			OsString::from("show").as_os_str(),
			OsString::from("--format=").as_os_str(),
			OsString::from("--color=always").as_os_str(),
			OsString::from("--expand-tabs=4").as_os_str(),
			OsString::from(commit_id.to_string()).as_os_str(),
			file_path.as_os_str(),
		])
		.current_dir(repo_path)
		.stdout(Stdio::piped())
		.spawn()
	{
		Ok(proc) => proc,
		Err(e) => return Text::raw(format!("git show: {}", e)),
	};
	let mut delta = Command::new("delta");
	delta.stdin(Stdio::from(git_show.stdout.unwrap()));

	let buf = match delta.output() {
		Ok(o) => {
			if o.status.success() {
				o.stdout
			} else {
				o.stderr
			}
		},
		Err(e) => {
			return Text::raw(e.to_string());
		},
	};
	match buf.into_text() {
		Ok(t) => t,
		Err(e) => Text::raw(format!("ansi_to_tui:\n{}", e)),
	}
}
