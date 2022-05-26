use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub struct StarshipPath<'a> {
    path: PathBuf,
    components: Vec<StarshipComponent<'a>>,
}

impl<'a> StarshipPath<'a> {
    pub fn new(path: &'a PathBuf, home_dir: &PathBuf) -> Self {
        let home_real_path = real_path(home_dir);
        Self {
            path: path.to_owned(),
            components: path
                .ancestors()
                .map(|component| {
                    let component_real_path = real_path(component);
                    log::warn!(
                        "componenet real path: {:?} - {:?}",
                        component_real_path,
                        home_real_path
                    );
                    StarshipComponent {
                        component: component.components().last(),
                        is_home: component_real_path == home_real_path,
                        is_repo: false,
                    }
                })
                .collect::<Vec<_>>(),
        }
    }
    fn find_component(&mut self, component_path: &PathBuf) -> Option<&mut StarshipComponent<'a>> {
        let top_level_real_path = real_path(component_path);
        // Walk ancestors to preserve logical path in `full_path`.
        // If we'd just `full_real_path.strip_prefix(top_level_real_path)`,
        // then it wouldn't preserve logical path. It would've returned physical path.
        for (i, ancestor) in self.path.ancestors().enumerate() {
            let ancestor_real_path = real_path(ancestor);
            if ancestor_real_path != top_level_real_path {
                continue;
            }
            let components: Vec<_> = self.path.components().collect();

            let component_index = components.len() - i - 1;

            return Some(&mut self.components[component_index]);
        }
        None
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
