use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub struct StarshipPath<'a> {
    path: PathBuf,
    components: Vec<StarshipComponent<'a>>,
}

impl<'a> StarshipPath<'a> {
    pub fn new(path: &'a PathBuf, home_dir: &PathBuf, repo_dir: Option<&PathBuf>) -> Self {
        let home_real_path = real_path(home_dir);
        let repo_real_path = repo_dir.map(|path| real_path(path));
        Self {
            path: path.to_owned(),
            components: path
                .ancestors()
                .map(|component| {
                    let component_real_path = real_path(component);
                    StarshipComponent {
                        component: component.components().last(),
                        is_home: component_real_path == home_real_path,
                        is_repo: Some(component_real_path) == repo_real_path,
                    }
                })
                .collect::<Vec<_>>(),
        }
    }
}

#[derive(Debug)]
struct StarshipComponent<'a> {
    component: Option<Component<'a>>,
    is_repo: bool,
    is_home: bool,
}

fn real_path<P: AsRef<Path>>(path: P) -> PathBuf {
    let path = path.as_ref();
    let mut buf = PathBuf::new();
    for component in path.components() {
        let next = buf.join(component);
        if let Ok(realpath) = next.read_link() {
            if realpath.is_absolute() {
                buf = realpath;
            } else {
                buf.push(realpath);
            }
        } else {
            buf = next;
        }
    }
    buf.canonicalize().unwrap_or_else(|_| path.into())
}
