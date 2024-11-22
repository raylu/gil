use git2::{Oid, Repository};
use std::env;

mod git;
mod terminal;

fn main() {
	let argv: Vec<String> = env::args().collect();
	if argv.len() > 3 {
		println!("usage: {} [rev] [--show]", argv[0].rsplit('/').next().unwrap());
		return;
	}

	let repo = match Repository::open_from_env() {
		Ok(repo) => repo,
		Err(err) => {
			println!("{}", err.message());
			return;
		},
	};
	let args = match get_commit_id(&repo, &argv[1..]) {
		Ok(commit_id) => commit_id,
		Err(err) => {
			println!("couldn't get commit: {}", err.message());
			return;
		},
	};

	let revwalk = match git::log(&repo, args.commit_id) {
		Ok(revwalk) => revwalk,
		Err(err) => {
			println!("couldn't log {}: {}", args.commit_id, err.message());
			return;
		},
	};
	let decorations = match git::decorations(&repo) {
		Ok(decorations) => decorations,
		Err(err) => {
			println!("couldn't get decorations: {}", err.message());
			return;
		},
	};

	let app = terminal::App::new(&repo, revwalk, decorations, args.show);
	let mut term = terminal::setup().unwrap();
	let res = terminal::run_app(&mut term, app);

	terminal::teardown(&mut term);
	if let Err(err) = res {
		println!("{:?}", err)
	}
}

struct Args {
	commit_id: Oid,
	show: bool,
}

fn get_commit_id(repo: &Repository, args: &[String]) -> Result<Args, git2::Error> {
	let mut show = false;
	let mut commit_id: Option<Oid> = None;
	for arg in args {
		if arg == "--show" {
			show = true;
		} else if commit_id.is_none() {
			commit_id = Some(repo.revparse_single(arg)?.id())
		} else {
			return Err(git2::Error::new(
				git2::ErrorCode::GenericError,
				git2::ErrorClass::None,
				format!("cannot pass multiple commits ({})", arg),
			));
		}
	}
	if commit_id.is_none() {
		commit_id = Some(repo.head()?.target().ok_or(git2::Error::new(
			git2::ErrorCode::GenericError,
			git2::ErrorClass::None,
			"couldn't resolve HEAD",
		))?);
	}
	Ok(Args {
		commit_id: commit_id.unwrap(),
		show,
	})
}
