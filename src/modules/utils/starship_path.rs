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
        let mut components = path
            .ancestors()
            .map(|ancestor| {
                let component_real_path = real_path(ancestor);
                StarshipComponent {
                    component: ancestor.components().last().unwrap(),
                    is_home: component_real_path == home_real_path,
                    is_repo: Some(component_real_path) == repo_real_path,
                }
            })
            .collect::<Vec<_>>();
        components.reverse();
        Self {
            path: path.to_owned(),
            components: components,
        }
    }
    fn truncate(&self, config: &'a DirectoryConfig) -> (usize, String) {
        let mut truncation: (usize, String) = (0, String::default());
        let path_length = self.components.len();

        // truncate to home
        if let Some(i) = self.components.iter().position(|x| x.is_home) {
            truncation = (i + 1, format!("{}/", config.home_symbol))
        };

        // truncate length
        if path_length - truncation.0 > config.truncation_length as usize {
            truncation = (
                (path_length - config.truncation_length as usize),
                String::from(config.truncation_symbol),
            )
        };

        // if config.truncate_to_repo {
        //     if let Some(i) = self.components.iter().position(|x| x.is_repo) {
        //         if truncation.0 > i {
        //             truncation = (i, "")
        //         }
        //     };
        // };
        truncation
    }
    pub fn display(&self, _config: &'a DirectoryConfig) -> String {
        let (trim_index, prefix) = self.truncate(_config);
        log::warn!("truncate: {:?} {:?}", trim_index, prefix);
        let path_components = self.components[trim_index..].iter();
        let path = String::from_iter(
            path_components
                .map(|x| x.get())
                // .map(|x| {
                //     log::warn!("componnent: {:?}", x);
                //     x
                // })
                .map(|x| format!("{}/", x)),
        );
        let path = format!("{}{}", prefix, path);
        path.strip_suffix('/').unwrap_or(&path).to_string()
    }
}

#[derive(Debug)]
struct StarshipComponent<'a> {
    component: Component<'a>,
    is_repo: bool,
    is_home: bool,
}

impl StarshipComponent<'_> {
    pub fn get(&self) -> String {
        String::from(self.component.as_os_str().to_string_lossy())
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
