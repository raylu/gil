use git2::{Oid, Repository, Revwalk};

pub struct CommitInfo {
	pub commit_id: Oid,
}

pub fn log(repo: &Repository, commit_id: Oid) -> Result<Revwalk, git2::Error> {
	let mut revwalk = repo.revwalk()?;
	revwalk.push(commit_id)?;
	return Ok(revwalk);
}
