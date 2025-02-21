use git2::Repository;
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
	let args = match parse_args(&argv[1..]) {
		Ok(args) => args,
		Err(err) => {
			println!("couldn't get commit: {}", err.message());
			return;
		},
	};

	let revwalk = match git::log(&repo, &args.revision_range) {
		Ok(revwalk) => revwalk,
		Err(err) => {
			println!("couldn't log {}: {}", args.revision_range, err.message());
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

	let term = terminal::setup().unwrap();
	let mut app = terminal::App::new(term, &repo, revwalk, decorations, args.revision_range, args.show);
	let res = app.run_app();

	app.teardown();
	if let Err(err) = res {
		println!("{:?}", err)
	}
}

struct Args {
	revision_range: String,
	show: bool,
}

fn parse_args(args: &[String]) -> Result<Args, git2::Error> {
	let mut show = false;
	let mut revision_range = None;
	for arg in args {
		if arg == "--show" {
			show = true;
		} else if revision_range.is_none() {
			revision_range = Some(arg.as_str())
		} else {
			return Err(git2::Error::new(
				git2::ErrorCode::GenericError,
				git2::ErrorClass::None,
				format!("cannot pass multiple commits ({})", arg),
			));
		}
	}
	Ok(Args {
		revision_range: revision_range.unwrap_or("HEAD").to_string(),
		show,
	})
}
