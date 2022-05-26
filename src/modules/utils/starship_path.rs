use crate::configs::directory::DirectoryConfig;
use std::cmp::Reverse;
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
                .map(|ancestor| {
                    let component_real_path = real_path(ancestor);
                    StarshipComponent {
                        component: ancestor.components().last(),
                        is_home: component_real_path == home_real_path,
                        is_repo: Some(component_real_path) == repo_real_path,
                    }
                })
                .collect::<Vec<_>>(),
        }
    }
    fn truncate(&self, config: &'a DirectoryConfig) -> (usize, &'a str) {
        let mut sort_options: Vec<(usize, &'a str)> = vec![(0, "")];

        if let Some(i) = self.components.iter().position(|x| x.is_home) {
            sort_options.push((i, config.home_symbol));
        };
        if config.truncate_to_repo {
            if let Some(i) = self.components.iter().position(|x| x.is_repo) {
                sort_options.push((i, ""));
            };
        };
        log::warn!("before sorting {:?}", sort_options);
        sort_options.sort_by_key(|k| Reverse(k.0));
        log::warn!("sorted {:?}", sort_options);
        sort_options[0]
    }
    pub fn display(&self, _config: &'a DirectoryConfig) -> String {
        let (start, prefix) = self.truncate(_config);
        let path_components = self.components[..start].iter();
        let path = String::from_iter(
            path_components
                .rev()
                .map(|x| x.get())
                .map(|x| format!("{}/", x)),
        );
        format!("{}{}", prefix, path).trim_end_matches('/').to_string()
    }
}

#[derive(Debug)]
struct StarshipComponent<'a> {
    component: Option<Component<'a>>,
    is_repo: bool,
    is_home: bool,
}

impl StarshipComponent<'_> {
    pub fn get(&self) -> String {
        String::from(self.component.unwrap().as_os_str().to_string_lossy())
    }
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
