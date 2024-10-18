use git2::{Oid, Repository};
use std::env;

mod git;
mod terminal;

fn main() {
	let args: Vec<String> = env::args().collect();
	if args.len() > 2 {
		println!("usage: {} [rev]", args[0].rsplit('/').next().unwrap());
		return;
	}

	let repo = match Repository::open_from_env() {
		Ok(repo) => repo,
		Err(err) => {
			println!("{}", err.message());
			return;
		}
	};
	let commit_id = match get_commit_id(&repo, &args) {
		Ok(commit_id) => commit_id,
		Err(err) => {
			println!("couldn't get commit: {}", err.message());
			return;
		}
	};

	let revwalk = match git::log(&repo, commit_id) {
		Ok(revwalk) => revwalk,
		Err(err) => {
			println!("couldn't log {}: {}", commit_id, err.message());
			return;
		}
	};

	let app = terminal::App::new(&repo, revwalk);
	let mut term = terminal::setup().unwrap();
	let res = terminal::run_app(&mut term, app);

	terminal::teardown(&mut term);
	if let Err(err) = res {
		println!("{:?}", err)
	}
}

fn get_commit_id(repo: &Repository, args: &[String]) -> Result<Oid, git2::Error> {
	if args.len() == 2 {
		Ok(repo.revparse_single(&args[1])?.id())
	} else {
		repo.head()?.target().ok_or(git2::Error::new(
			git2::ErrorCode::GenericError,
			git2::ErrorClass::None,
			"couldn't resolve HEAD",
		))
	}
}
