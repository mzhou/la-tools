use std::error::Error;
use std::ffi::OsString;

const APPLET_NAMES: &[&str] = &[
    EXTRACT_GIT_OBJECT,
    HASH_GIT_OBJECT,
    MAKE_GIT_OBJECT,
    PATCH_GIT_INDEX,
];
const EXTRACT_GIT_OBJECT: &str = "extract-git-object";
const HASH_GIT_OBJECT: &str = "hash-git-object";
const MAKE_GIT_OBJECT: &str = "make-git-object";
const PATCH_GIT_INDEX: &str = "patch-git-index";

pub fn try_main<I, T>(itr: I) -> Result<i32, Box<dyn Error>>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let args: Vec<OsString> = itr.into_iter().map(|i| i.into()).collect();

    for skip in 0..=1 {
        if args.len() < skip + 1 {
            usage();
            return Ok(127);
        }

        let applet_name = args[skip].to_string_lossy();

        if let Some(r) = try_dispatch(&applet_name, &args[skip..]) {
            return r;
        }
    }

    Ok(0)
}

fn try_dispatch(applet_name: &str, args: &[OsString]) -> Option<Result<i32, Box<dyn Error>>> {
    match applet_name {
        EXTRACT_GIT_OBJECT => Some(extract_git_object::try_main(args)),
        HASH_GIT_OBJECT => Some(hash_git_object::try_main(args)),
        MAKE_GIT_OBJECT => Some(make_git_object::try_main(args)),
        _ => None,
    }
}

fn usage() {
    eprintln!("Usage: <applet> ...");
    eprintln!("Applets:");
    for applet_name in APPLET_NAMES.iter() {
        eprintln!("    {}", applet_name);
    }
}
