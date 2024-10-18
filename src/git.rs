use git2::{Diff, DiffStatsFormat, Oid, Repository, Revwalk};

pub struct CommitInfo<'repo> {
	pub commit_id: Oid,
	pub author: String,
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
	let tree: git2::Tree;
	let parent_tree = match commit.parent(0) {
		Ok(parent) => {
			tree = parent.tree()?;
			Some(&tree)
		}
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
		author: format!(
			"{} <{}>",
			author.name().unwrap_or_default(),
			author.email().unwrap_or_default()
		),
		summary: commit.summary().unwrap_or_default().to_owned(),
		message: commit.message().unwrap_or_default().to_owned(),
		patch,
		stats,
	}));
}
