use git2::Repository;
use std::env;

mod git;
mod terminal;

fn main() {
	let args: Vec<String> = env::args().collect();
	if args.len() > 2 {
		println!("usage: {} [rev]", args[0].rsplit('/').next().unwrap());
		return;
	}

	let repo = Repository::open_from_env().unwrap();
	let commit_id = if args.len() == 2 {
		repo.revparse_single(&args[1]).unwrap().id()
	} else {
		repo.head().unwrap().target().unwrap()
	};
	let app = terminal::App::new(&repo, commit_id);
	let mut term = terminal::setup().unwrap();
	let res = terminal::run_app(&mut term, app);

	terminal::teardown(&mut term);
	if let Err(err) = res {
		println!("{:?}", err)
	}
}
