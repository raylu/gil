use git2::{Oid, Repository, Revwalk};

pub struct CommitInfo {
	pub commit_id: Oid,
	pub author: String,
	pub summary: String,
	pub message: String,
}

pub fn log(repo: &Repository, commit_id: Oid) -> Result<Revwalk, git2::Error> {
	let mut revwalk = repo.revwalk()?;
	revwalk.push(commit_id)?;
	return Ok(revwalk);
}

pub fn next_commit(repo: &Repository, revwalk: &mut Revwalk) -> Result<Option<CommitInfo>, git2::Error> {
	let commit_id = match revwalk.next() {
		None => return Ok(None),
		Some(result) => result?,
	};
	let commit = repo.find_commit(commit_id)?;
	let author = commit.author();
	return Ok(Some(CommitInfo {
		commit_id,
		author: format!(
			"{} <{}>",
			author.name().unwrap_or_default(),
			author.email().unwrap_or_default()
		),
		summary: commit.summary().unwrap_or_default().to_owned(),
		message: commit.message().unwrap_or_default().to_owned(),
	}));
}
