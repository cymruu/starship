#[cfg(not(target_os = "windows"))]
use super::utils::directory_nix as directory_utils;
#[cfg(target_os = "windows")]
use super::utils::directory_win as directory_utils;
use super::utils::path::PathExt as SPathExt;
use indexmap::IndexMap;
use path_slash::PathExt;
use std::iter::FromIterator;
use std::path::{Component, Path, PathBuf};
use unicode_segmentation::UnicodeSegmentation;

use super::{Context, Module};

use super::utils::directory::truncate;
use crate::config::RootModuleConfig;
use crate::configs::directory::DirectoryConfig;
use crate::formatter::StringFormatter;

#[derive(Debug)]
struct StarshipPath<'a> {
    path: &'a PathBuf,
    components: Vec<StarshipComponent<'a>>,
}

impl<'a> StarshipPath<'a> {
    pub fn to_string_lossy(&self) -> std::borrow::Cow<'_, str> {
        self.path.to_string_lossy()
    }
    pub fn find_repo(&mut self, repo_path: &Path) {
        if let Some(repo_component) = self.find_component(repo_path) {
            repo_component.is_repo = true;
        };
    }
    pub fn find_home(&mut self, home_path: &Path) {
        if let Some(home_component) = self.find_component(home_path) {
            home_component.is_home = true;
        };
    }
    fn find_component(&mut self, component_path: &Path) -> Option<&mut StarshipComponent<'a>> {
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
    pub fn get_format_string(&self, config: &DirectoryConfig) -> String {
        let (start_index, prefix) = self.get_path_start_index_and_prefix(config);
        let path_meta = self.components[start_index..]
            .iter()
            .map(|x| x.get_format_string(config));

        let mut path = String::from_iter(path_meta);
        log::warn!("path1: {}", path);
        if !prefix.is_empty() {
            path = format!("{}{}", prefix, path);
        };
        log::warn!("path with prefix: {}", path);

        if (self.components.len() - start_index) > 1{
            path = path.strip_suffix('/').unwrap_or(&path).to_string();
        };

        path
    }
    fn get_path_start_index_and_prefix(&self, config: &DirectoryConfig) -> (usize, String) {
        let repo_index = self.components.iter().position(|x| x.is_repo);
        let (start_index, prefix) = if config.truncate_to_repo && repo_index.is_some() {
            (repo_index, "")
        } else {
            let home_index = self.components.iter().position(|x| x.is_home);
            let prefix = if home_index.is_some() {
                config.home_symbol
            } else {
                ""
            };
            (home_index.map(|x| x + 1), prefix)
        };
        let start_index = start_index.unwrap_or(0);

        let (start_index, prefix) = if config.truncation_length != 0
            && (self.components.len() - start_index) > config.truncation_length as usize
        {
            (
                self.components.len() - config.truncation_length as usize,
                config.truncation_symbol,
            )
        } else {
            (start_index, prefix)
        };
        (start_index, String::from(prefix))
    }
}

impl<'a> From<&'a PathBuf> for StarshipPath<'a> {
    fn from(path: &'a PathBuf) -> StarshipPath<'a> {
        let components: Vec<_> = path
            .components()
            .map(|x| StarshipComponent {
                component: x,
                is_repo: false,
                is_home: false,
            })
            .collect();
        StarshipPath { path, components }
    }
}

#[derive(Debug)]
struct StarshipComponent<'a> {
    component: Component<'a>,
    is_repo: bool,
    is_home: bool,
}

impl StarshipComponent<'_> {
    pub fn get_format_string(&self, config: &DirectoryConfig) -> String {
        let component_dir = self.component.as_os_str().to_string_lossy();
        let component_suffix = match self.component {
            Component::RootDir => "",
            _ => "/",
        };
        let component_format_string = match self.is_repo {
            true => format!("[{}]({})", component_dir, config.repo_root_style,),
            false => format!("{}", component_dir),
        };

        format!("{}{}", component_format_string, component_suffix)
    }
}

/// Creates a module with the current logical or physical directory
///
/// Will perform path contraction, substitution, and truncation.
///
/// **Contraction**
/// - Paths beginning with the home directory or with a git repo right inside
///   the home directory will be contracted to `~`, or the set HOME_SYMBOL
/// - Paths containing a git repo will contract to begin at the repo root
///
/// **Substitution**
/// Paths will undergo user-provided substitutions of substrings
///
/// **Truncation**
/// Paths will be limited in length to `3` path components by default.
pub fn module<'a>(context: &'a Context) -> Option<Module<'a>> {
    let mut module = context.new_module("directory");
    let config: DirectoryConfig = DirectoryConfig::try_load(module.config);

    let home_symbol = String::from(config.home_symbol);
    let home_dir = context
        .get_home()
        .expect("Unable to determine HOME_DIR for user");
    let physical_dir = &context.current_dir;
    let display_dir = if config.use_logical_path {
        &context.logical_dir
    } else {
        &context.current_dir
    };

    log::debug!("Home dir: {:?}", &home_dir);
    log::debug!("Physical dir: {:?}", &physical_dir);
    log::debug!("Display dir: {:?}", &display_dir);

    let mut starship_path = StarshipPath::from(display_dir);
    starship_path.find_home(&home_dir);

    if let Some(repo) = context.get_repo().ok() {
        if let Some(repo_path) = repo.root.as_ref() {
            starship_path.find_repo(repo_path);
        };
    };

    log::warn!("{:?}, {:?}", starship_path, starship_path.components);

    let dir_string = format!("{}", display_dir.to_string_lossy());

    #[cfg(windows)]
    let dir_string = remove_extended_path_prefix(display_dir);

    log::warn!("dir_string: {}", dir_string);

    let path_meta = starship_path.get_format_string(&config);
    let lock_symbol = String::from(config.read_only);

    let parsed = StringFormatter::new(config.format).and_then(|formatter| {
        formatter
            .map_meta(|var, _| match var {
                "path" => Some(&path_meta),
                _ => None,
            })
            .map_style(|variable| match variable {
                "style" => Some(Ok(config.style)),
                "repo_root_style" => Some(Ok(config.repo_root_style)),
                "read_only_style" => Some(Ok(config.read_only_style)),
                _ => None,
            })
            .map(|variable| match variable {
                "read_only" => {
                    if is_readonly_dir(physical_dir) {
                        Some(Ok(&lock_symbol))
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .parse(None, Some(context))
    });

    module.set_segments(match parsed {
        Ok(segments) => segments,
        Err(error) => {
            log::warn!("Error in module `directory`:\n{}", error);
            return None;
        }
    });

    Some(module)
}

#[cfg(windows)]
fn remove_extended_path_prefix(path: String) -> String {
    fn try_trim_prefix<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
        if !s.starts_with(prefix) {
            return None;
        }
        Some(&s[prefix.len()..])
    }
    // Trim any Windows extended-path prefix from the display path
    if let Some(unc) = try_trim_prefix(&path, r"\\?\UNC\") {
        return format!(r"\\{}", unc);
    }
    if let Some(p) = try_trim_prefix(&path, r"\\?\") {
        return p.to_string();
    }
    path
}

fn is_truncated(path: &str, home_symbol: &str) -> bool {
    !(path.starts_with(&home_symbol)
        || PathBuf::from(path).has_root()
        || (cfg!(target_os = "windows") && PathBuf::from(String::from(path) + r"\").has_root()))
}

fn is_readonly_dir(path: &Path) -> bool {
    match directory_utils::is_write_allowed(path) {
        Ok(res) => !res,
        Err(e) => {
            log::debug!(
                "Failed to determine read only status of directory '{:?}': {}",
                path,
                e
            );
            false
        }
    }
}

/// Contract the root component of a path
///
/// Replaces the `top_level_path` in a given `full_path` with the provided
/// `top_level_replacement`.
fn contract_path(full_path: &Path, top_level_path: &Path, top_level_replacement: &str) -> String {
    if !full_path.normalised_starts_with(top_level_path) {
        return full_path.to_slash_lossy();
    }

    if full_path.normalised_equals(top_level_path) {
        return top_level_replacement.to_string();
    }

    // Because we've done a normalised path comparison above
    // we can safely ignore the Prefix components when doing this
    // strip_prefix operation.
    let sub_path = full_path
        .without_prefix()
        .strip_prefix(top_level_path.without_prefix())
        .unwrap_or(full_path);

    format!(
        "{replacement}{separator}{path}",
        replacement = top_level_replacement,
        separator = "/",
        path = sub_path.to_slash_lossy()
    )
}

/// Contract the root component of a path based on the real path
///
/// Replaces the `top_level_path` in a given `full_path` with the provided
/// `top_level_replacement` by walking ancestors and comparing its real path.
fn contract_repo_path(full_path: &Path, top_level_path: &Path) -> Option<String> {
    let top_level_real_path = real_path(top_level_path);
    // Walk ancestors to preserve logical path in `full_path`.
    // If we'd just `full_real_path.strip_prefix(top_level_real_path)`,
    // then it wouldn't preserve logical path. It would've returned physical path.
    for (i, ancestor) in full_path.ancestors().enumerate() {
        let ancestor_real_path = real_path(ancestor);
        if ancestor_real_path != top_level_real_path {
            continue;
        }

        let components: Vec<_> = full_path.components().collect();
        let repo_name = components[components.len() - i - 1]
            .as_os_str()
            .to_string_lossy();

        if i == 0 {
            return Some(repo_name.to_string());
        }

        let path = PathBuf::from_iter(&components[components.len() - i..]);
        return Some(format!(
            "{repo_name}{separator}{path}",
            repo_name = repo_name,
            separator = "/",
            path = path.to_slash_lossy()
        ));
    }
    None
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

/// Perform a list of string substitutions on the path
///
/// Given a list of (from, to) pairs, this will perform the string
/// substitutions, in order, on the path. Any non-pair of strings is ignored.
fn substitute_path(dir_string: String, substitutions: &IndexMap<String, &str>) -> String {
    let mut substituted_dir = dir_string;
    for substitution_pair in substitutions {
        substituted_dir = substituted_dir.replace(substitution_pair.0, substitution_pair.1);
    }
    substituted_dir
}

/// Takes part before contracted path and replaces it with fish style path
///
/// Will take the first letter of each directory before the contracted path and
/// use that in the path instead. See the following example.
///
/// Absolute Path: `/Users/Bob/Projects/work/a_repo`
/// Contracted Path: `a_repo`
/// With Fish Style: `~/P/w/a_repo`
///
/// Absolute Path: `/some/Path/not/in_a/repo/but_nested`
/// Contracted Path: `in_a/repo/but_nested`
/// With Fish Style: `/s/P/n/in_a/repo/but_nested`
fn to_fish_style(pwd_dir_length: usize, dir_string: String, truncated_dir_string: &str) -> String {
    let replaced_dir_string = dir_string.trim_end_matches(truncated_dir_string).to_owned();
    let components = replaced_dir_string.split('/').collect::<Vec<&str>>();

    if components.is_empty() {
        return replaced_dir_string;
    }

    components
        .into_iter()
        .map(|word| -> String {
            let chars = UnicodeSegmentation::graphemes(word, true).collect::<Vec<&str>>();
            match word {
                "" => "".to_string(),
                _ if chars.len() <= pwd_dir_length => word.to_string(),
                _ if word.starts_with('.') => chars[..=pwd_dir_length].join(""),
                _ => chars[..pwd_dir_length].join(""),
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::ModuleRenderer;
    use crate::utils::create_command;
    use crate::utils::home_dir;
    use ansi_term::Color;
    #[cfg(not(target_os = "windows"))]
    use std::os::unix::fs::symlink;
    #[cfg(target_os = "windows")]
    use std::os::windows::fs::symlink_dir as symlink;
    use std::path::Path;
    use std::{fs, io};
    use tempfile::TempDir;

    #[test]
    fn contract_home_directory() {
        let full_path = Path::new("/Users/astronaut/schematics/rocket");
        let home = Path::new("/Users/astronaut");

        let output = contract_path(full_path, home, "~");
        assert_eq!(output, "~/schematics/rocket");
    }

    #[test]
    fn contract_repo_directory() -> io::Result<()> {
        let tmp_dir = TempDir::new_in(home_dir().unwrap().as_path())?;
        let repo_dir = tmp_dir.path().join("dev").join("rocket-controls");
        let src_dir = repo_dir.join("src");
        fs::create_dir_all(&src_dir)?;
        init_repo(&repo_dir)?;

        let src_variations = [src_dir.clone(), src_dir.canonicalize().unwrap()];
        let repo_variations = [repo_dir.clone(), repo_dir.canonicalize().unwrap()];
        for src_dir in &src_variations {
            for repo_dir in &repo_variations {
                let output = contract_repo_path(src_dir, repo_dir);
                assert_eq!(output, Some("rocket-controls/src".to_string()));
            }
        }

        tmp_dir.close()
    }

    #[test]
    #[cfg(windows)]
    fn contract_windows_style_home_directory() {
        let path_variations = [
            r"\\?\C:\Users\astronaut\schematics\rocket",
            r"C:\Users\astronaut\schematics\rocket",
        ];
        let home_path_variations = [r"\\?\C:\Users\astronaut", r"C:\Users\astronaut"];
        for path in &path_variations {
            for home_path in &home_path_variations {
                let path = Path::new(path);
                let home_path = Path::new(home_path);

                let output = contract_path(path, home_path, "~");
                assert_eq!(output, "~/schematics/rocket");
            }
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn contract_windows_style_repo_directory() {
        let full_path = Path::new("C:\\Users\\astronaut\\dev\\rocket-controls\\src");
        let repo_root = Path::new("C:\\Users\\astronaut\\dev\\rocket-controls");

        let output = contract_path(full_path, repo_root, "rocket-controls");
        assert_eq!(output, "rocket-controls/src");
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn contract_windows_style_no_top_level_directory() {
        let full_path = Path::new("C:\\Some\\Other\\Path");
        let top_level_path = Path::new("C:\\Users\\astronaut");

        let output = contract_path(full_path, top_level_path, "~");
        assert_eq!(output, "C:/Some/Other/Path");
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn contract_windows_style_root_directory() {
        let full_path = Path::new("C:\\");
        let top_level_path = Path::new("C:\\Users\\astronaut");

        let output = contract_path(full_path, top_level_path, "~");
        assert_eq!(output, "C:");
    }

    #[test]
    fn substitute_prefix_and_middle() {
        let full_path = "/absolute/path/foo/bar/baz";
        let mut substitutions = IndexMap::new();
        substitutions.insert("/absolute/path".to_string(), "");
        substitutions.insert("/bar/".to_string(), "/");

        let output = substitute_path(full_path.to_string(), &substitutions);
        assert_eq!(output, "/foo/baz");
    }

    #[test]
    fn fish_style_with_user_home_contracted_path() {
        let path = "~/starship/engines/booster/rocket";
        let output = to_fish_style(1, path.to_string(), "engines/booster/rocket");
        assert_eq!(output, "~/s/");
    }

    #[test]
    fn fish_style_with_user_home_contracted_path_and_dot_dir() {
        let path = "~/.starship/engines/booster/rocket";
        let output = to_fish_style(1, path.to_string(), "engines/booster/rocket");
        assert_eq!(output, "~/.s/");
    }

    #[test]
    fn fish_style_with_no_contracted_path() {
        // `truncation_length = 2`
        let path = "/absolute/Path/not/in_a/repo/but_nested";
        let output = to_fish_style(1, path.to_string(), "repo/but_nested");
        assert_eq!(output, "/a/P/n/i/");
    }

    #[test]
    fn fish_style_with_pwd_dir_len_no_contracted_path() {
        // `truncation_length = 2`
        let path = "/absolute/Path/not/in_a/repo/but_nested";
        let output = to_fish_style(2, path.to_string(), "repo/but_nested");
        assert_eq!(output, "/ab/Pa/no/in/");
    }

    #[test]
    fn fish_style_with_duplicate_directories() {
        let path = "~/starship/tmp/C++/C++/C++";
        let output = to_fish_style(1, path.to_string(), "C++");
        assert_eq!(output, "~/s/t/C/C/");
    }

    #[test]
    fn fish_style_with_unicode() {
        let path = "~/starship/tmp/目录/a̐éö̲/目录";
        let output = to_fish_style(1, path.to_string(), "目录");
        assert_eq!(output, "~/s/t/目/a̐/");
    }

    fn init_repo(path: &Path) -> io::Result<()> {
        create_command("git")?
            .args(&["init"])
            .current_dir(path)
            .output()
            .map(|_| ())
    }

    fn make_known_tempdir(root: &Path) -> io::Result<(TempDir, String)> {
        fs::create_dir_all(root)?;
        let dir = TempDir::new_in(root)?;
        let path = dir
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        Ok((dir, path))
    }

    #[cfg(not(target_os = "windows"))]
    mod linux {
        use super::*;

        #[test]
        #[ignore]
        fn symlinked_subdirectory_git_repo_out_of_tree() -> io::Result<()> {
            let tmp_dir = TempDir::new_in(home_dir().unwrap().as_path())?;
            let repo_dir = tmp_dir.path().join("above-repo").join("rocket-controls");
            let src_dir = repo_dir.join("src/meters/fuel-gauge");
            let symlink_dir = tmp_dir.path().join("fuel-gauge");
            fs::create_dir_all(&src_dir)?;
            init_repo(&repo_dir)?;
            symlink(&src_dir, &symlink_dir)?;

            let actual = ModuleRenderer::new("directory")
                .env("HOME", tmp_dir.path().to_str().unwrap())
                .path(symlink_dir)
                .collect();
            let expected = Some(format!("{} ", Color::Cyan.bold().paint("~/fuel-gauge")));

            assert_eq!(expected, actual);

            tmp_dir.close()
        }

        #[test]
        #[ignore]
        fn git_repo_in_home_directory_truncate_to_repo_true() -> io::Result<()> {
            let tmp_dir = TempDir::new_in(home_dir().unwrap().as_path())?;
            let dir = tmp_dir.path().join("src/fuel-gauge");
            fs::create_dir_all(&dir)?;
            init_repo(tmp_dir.path())?;

            let actual = ModuleRenderer::new("directory")
                .config(toml::toml! {
                    [directory]
                    // `truncate_to_repo = true` should attempt to display the truncated path
                    truncate_to_repo = true
                    truncation_length = 5
                })
                .path(dir)
                .env("HOME", tmp_dir.path().to_str().unwrap())
                .collect();
            let expected = Some(format!("{} ", Color::Cyan.bold().paint("~/src/fuel-gauge")));

            assert_eq!(expected, actual);

            tmp_dir.close()
        }

        #[test]
        #[ignore]
        fn directory_in_root() {
            let actual = ModuleRenderer::new("directory").path("/etc").collect();
            let expected = Some(format!(
                "{}{} ",
                Color::Cyan.bold().paint("/etc"),
                Color::Red.normal().paint("🔒")
            ));

            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn home_directory_default_home_symbol() {
        let actual = ModuleRenderer::new("directory")
            .path(home_dir().unwrap())
            .collect();
        let expected = Some(format!("{} ", Color::Cyan.bold().paint("~")));

        assert_eq!(expected, actual);
    }

    #[test]
    fn home_directory_custom_home_symbol() {
        let actual = ModuleRenderer::new("directory")
            .path(home_dir().unwrap())
            .config(toml::toml! {
                [directory]
                home_symbol = "🚀"
            })
            .collect();
        let expected = Some(format!("{} ", Color::Cyan.bold().paint("🚀")));

        assert_eq!(expected, actual);
    }

    #[test]
    fn home_directory_custom_home_symbol_subdirectories() {
        let actual = ModuleRenderer::new("directory")
            .path(home_dir().unwrap().join("path/subpath"))
            .config(toml::toml! {
                [directory]
                home_symbol = "🚀"
            })
            .collect();
        let expected = Some(format!("{} ", Color::Cyan.bold().paint("🚀/path/subpath")));

        assert_eq!(expected, actual);
    }

    #[test]
    fn substituted_truncated_path() {
        let actual = ModuleRenderer::new("directory")
            .path("/some/long/network/path/workspace/a/b/c/dev")
            .config(toml::toml! {
                [directory]
                truncation_length = 4
                [directory.substitutions]
                "/some/long/network/path" = "/some/net"
                "a/b/c" = "d"
            })
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint("net/workspace/d/dev")
        ));

        assert_eq!(expected, actual);
    }

    #[test]
    fn substitution_order() {
        let actual = ModuleRenderer::new("directory")
            .path("/path/to/sub")
            .config(toml::toml! {
                [directory.substitutions]
                "/path/to/sub" = "/correct/order"
                "/to/sub" = "/wrong/order"
            })
            .collect();
        let expected = Some(format!("{} ", Color::Cyan.bold().paint("/correct/order")));

        assert_eq!(expected, actual);
    }

    #[test]
    fn strange_substitution() {
        let strange_sub = "/\\/;,!";
        let actual = ModuleRenderer::new("directory")
            .path("/foo/bar/regular/path")
            .config(toml::toml! {
                [directory]
                truncation_length = 0
                fish_style_pwd_dir_length = 2 // Overridden by substitutions
                [directory.substitutions]
                "regular" = strange_sub
            })
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan
                .bold()
                .paint(format!("/foo/bar/{}/path", strange_sub))
        ));

        assert_eq!(expected, actual);
    }

    #[test]
    fn directory_in_home() -> io::Result<()> {
        let (tmp_dir, name) = make_known_tempdir(home_dir().unwrap().as_path())?;
        let dir = tmp_dir.path().join("starship");
        fs::create_dir_all(&dir)?;

        let actual = ModuleRenderer::new("directory").path(dir).collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint(format!("~/{}/starship", name))
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    fn truncated_directory_in_home() -> io::Result<()> {
        let (tmp_dir, name) = make_known_tempdir(home_dir().unwrap().as_path())?;
        let dir = tmp_dir.path().join("engine/schematics");
        fs::create_dir_all(&dir)?;

        let actual = ModuleRenderer::new("directory").path(dir).collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan
                .bold()
                .paint(format!("{}/engine/schematics", name))
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    fn fish_directory_in_home() -> io::Result<()> {
        let (tmp_dir, name) = make_known_tempdir(home_dir().unwrap().as_path())?;
        let dir = tmp_dir.path().join("starship/schematics");
        fs::create_dir_all(&dir)?;

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                truncation_length = 1
                fish_style_pwd_dir_length = 2
            })
            .path(&dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan
                .bold()
                .paint(format!("~/{}/st/schematics", name.split_at(3).0))
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    fn root_directory() {
        // Note: We have disable the read_only settings here due to false positives when running
        // the tests on Windows as a non-admin.
        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                read_only = ""
                read_only_style = ""
            })
            .path("/")
            .collect();
        let expected = Some(format!("{} ", Color::Cyan.bold().paint("/")));

        assert_eq!(expected, actual);
    }

    #[test]
    fn truncated_directory_in_root() -> io::Result<()> {
        let (tmp_dir, name) = make_known_tempdir(Path::new("/tmp"))?;
        let dir = tmp_dir.path().join("thrusters/rocket");
        fs::create_dir_all(&dir)?;

        let actual = ModuleRenderer::new("directory").path(dir).collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan
                .bold()
                .paint(format!("{}/thrusters/rocket", name))
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    fn truncated_directory_config_large() -> io::Result<()> {
        let (tmp_dir, _) = make_known_tempdir(Path::new("/tmp"))?;
        let dir = tmp_dir.path().join("thrusters/rocket");
        fs::create_dir_all(&dir)?;

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                truncation_length = 100
            })
            .path(&dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan
                .bold()
                .paint(truncate(dir.to_slash_lossy(), 100))
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    fn fish_style_directory_config_large() -> io::Result<()> {
        let (tmp_dir, _) = make_known_tempdir(Path::new("/tmp"))?;
        let dir = tmp_dir.path().join("thrusters/rocket");
        fs::create_dir_all(&dir)?;

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                truncation_length = 1
                fish_style_pwd_dir_length = 100
            })
            .path(&dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan
                .bold()
                .paint(to_fish_style(100, dir.to_slash_lossy(), ""))
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    fn truncated_directory_config_small() -> io::Result<()> {
        let (tmp_dir, name) = make_known_tempdir(Path::new("/tmp"))?;
        let dir = tmp_dir.path().join("rocket");
        fs::create_dir_all(&dir)?;

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                truncation_length = 2
            })
            .path(dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint(format!("{}/rocket", name))
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    fn fish_directory_config_small() -> io::Result<()> {
        let (tmp_dir, _) = make_known_tempdir(Path::new("/tmp"))?;
        let dir = tmp_dir.path().join("thrusters/rocket");
        fs::create_dir_all(&dir)?;

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                truncation_length = 2
                fish_style_pwd_dir_length = 1
            })
            .path(&dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint(format!(
                "{}/thrusters/rocket",
                to_fish_style(1, dir.to_slash_lossy(), "/thrusters/rocket")
            ))
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    #[ignore]
    fn git_repo_root() -> io::Result<()> {
        let tmp_dir = TempDir::new()?;
        let repo_dir = tmp_dir.path().join("rocket-controls");
        fs::create_dir(&repo_dir)?;
        init_repo(&repo_dir).unwrap();

        let actual = ModuleRenderer::new("directory").path(repo_dir).collect();
        let expected = Some(format!("{} ", Color::Cyan.bold().paint("rocket-controls")));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    #[ignore]
    fn directory_in_git_repo() -> io::Result<()> {
        let tmp_dir = TempDir::new()?;
        let repo_dir = tmp_dir.path().join("rocket-controls");
        let dir = repo_dir.join("src");
        fs::create_dir_all(&dir)?;
        init_repo(&repo_dir).unwrap();

        let actual = ModuleRenderer::new("directory").path(dir).collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint("rocket-controls/src")
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    #[ignore]
    fn truncated_directory_in_git_repo() -> io::Result<()> {
        let tmp_dir = TempDir::new()?;
        let repo_dir = tmp_dir.path().join("rocket-controls");
        let dir = repo_dir.join("src/meters/fuel-gauge");
        fs::create_dir_all(&dir)?;
        init_repo(&repo_dir).unwrap();

        let actual = ModuleRenderer::new("directory").path(dir).collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint("src/meters/fuel-gauge")
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    #[ignore]
    fn directory_in_git_repo_truncate_to_repo_false() -> io::Result<()> {
        let tmp_dir = TempDir::new()?;
        let repo_dir = tmp_dir.path().join("above-repo").join("rocket-controls");
        let dir = repo_dir.join("src/meters/fuel-gauge");
        fs::create_dir_all(&dir)?;
        init_repo(&repo_dir).unwrap();

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                // Don't truncate the path at all.
                truncation_length = 5
                truncate_to_repo = false
            })
            .path(dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan
                .bold()
                .paint("above-repo/rocket-controls/src/meters/fuel-gauge")
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    #[ignore]
    fn fish_path_directory_in_git_repo_truncate_to_repo_false() -> io::Result<()> {
        let (tmp_dir, _) = make_known_tempdir(Path::new("/tmp"))?;
        let repo_dir = tmp_dir.path().join("above-repo").join("rocket-controls");
        let dir = repo_dir.join("src/meters/fuel-gauge");
        fs::create_dir_all(&dir)?;
        init_repo(&repo_dir).unwrap();

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                // Don't truncate the path at all.
                truncation_length = 5
                truncate_to_repo = false
                fish_style_pwd_dir_length = 1
            })
            .path(dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint(format!(
                "{}/above-repo/rocket-controls/src/meters/fuel-gauge",
                to_fish_style(1, tmp_dir.path().to_slash_lossy(), "")
            ))
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    #[ignore]
    fn fish_path_directory_in_git_repo_truncate_to_repo_true() -> io::Result<()> {
        let (tmp_dir, _) = make_known_tempdir(Path::new("/tmp"))?;
        let repo_dir = tmp_dir.path().join("above-repo").join("rocket-controls");
        let dir = repo_dir.join("src/meters/fuel-gauge");
        fs::create_dir_all(&dir)?;
        init_repo(&repo_dir).unwrap();

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                // `truncate_to_repo = true` should display the truncated path
                truncation_length = 5
                truncate_to_repo = true
                fish_style_pwd_dir_length = 1
            })
            .path(dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint(format!(
                "{}/rocket-controls/src/meters/fuel-gauge",
                to_fish_style(1, tmp_dir.path().join("above-repo").to_slash_lossy(), "")
            ))
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    #[ignore]
    fn directory_in_git_repo_truncate_to_repo_true() -> io::Result<()> {
        let (tmp_dir, _) = make_known_tempdir(Path::new("/tmp"))?;
        let repo_dir = tmp_dir.path().join("above-repo").join("rocket-controls");
        let dir = repo_dir.join("src/meters/fuel-gauge");
        fs::create_dir_all(&dir)?;
        init_repo(&repo_dir).unwrap();

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                // `truncate_to_repo = true` should display the truncated path
                truncation_length = 5
                truncate_to_repo = true
            })
            .path(dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan
                .bold()
                .paint("rocket-controls/src/meters/fuel-gauge")
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    #[ignore]
    fn symlinked_git_repo_root() -> io::Result<()> {
        let (tmp_dir, _) = make_known_tempdir(Path::new("/tmp"))?;
        let repo_dir = tmp_dir.path().join("rocket-controls");
        let symlink_dir = tmp_dir.path().join("rocket-controls-symlink");
        fs::create_dir(&repo_dir)?;
        init_repo(&repo_dir).unwrap();
        symlink(&repo_dir, &symlink_dir)?;

        let actual = ModuleRenderer::new("directory").path(symlink_dir).collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint("rocket-controls-symlink")
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    #[ignore]
    fn directory_in_symlinked_git_repo() -> io::Result<()> {
        let (tmp_dir, _) = make_known_tempdir(Path::new("/tmp"))?;
        let repo_dir = tmp_dir.path().join("rocket-controls");
        let src_dir = repo_dir.join("src");
        let symlink_dir = tmp_dir.path().join("rocket-controls-symlink");
        let symlink_src_dir = symlink_dir.join("src");
        fs::create_dir_all(&src_dir)?;
        init_repo(&repo_dir).unwrap();
        symlink(&repo_dir, &symlink_dir)?;

        let actual = ModuleRenderer::new("directory")
            .path(symlink_src_dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint("rocket-controls-symlink/src")
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    #[ignore]
    fn truncated_directory_in_symlinked_git_repo() -> io::Result<()> {
        let (tmp_dir, _) = make_known_tempdir(Path::new("/tmp"))?;
        let repo_dir = tmp_dir.path().join("rocket-controls");
        let src_dir = repo_dir.join("src/meters/fuel-gauge");
        let symlink_dir = tmp_dir.path().join("rocket-controls-symlink");
        let symlink_src_dir = symlink_dir.join("src/meters/fuel-gauge");
        fs::create_dir_all(&src_dir)?;
        init_repo(&repo_dir).unwrap();
        symlink(&repo_dir, &symlink_dir)?;

        let actual = ModuleRenderer::new("directory")
            .path(symlink_src_dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint("src/meters/fuel-gauge")
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    #[ignore]
    fn directory_in_symlinked_git_repo_truncate_to_repo_false() -> io::Result<()> {
        let (tmp_dir, _) = make_known_tempdir(Path::new("/tmp"))?;
        let repo_dir = tmp_dir.path().join("above-repo").join("rocket-controls");
        let src_dir = repo_dir.join("src/meters/fuel-gauge");
        let symlink_dir = tmp_dir
            .path()
            .join("above-repo")
            .join("rocket-controls-symlink");
        let symlink_src_dir = symlink_dir.join("src/meters/fuel-gauge");
        fs::create_dir_all(&src_dir)?;
        init_repo(&repo_dir).unwrap();
        symlink(&repo_dir, &symlink_dir)?;

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                // Don't truncate the path at all.
                truncation_length = 5
                truncate_to_repo = false
            })
            .path(symlink_src_dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan
                .bold()
                .paint("above-repo/rocket-controls-symlink/src/meters/fuel-gauge")
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    #[ignore]
    fn fish_path_directory_in_symlinked_git_repo_truncate_to_repo_false() -> io::Result<()> {
        let (tmp_dir, _) = make_known_tempdir(Path::new("/tmp"))?;
        let repo_dir = tmp_dir.path().join("above-repo").join("rocket-controls");
        let src_dir = repo_dir.join("src/meters/fuel-gauge");
        let symlink_dir = tmp_dir
            .path()
            .join("above-repo")
            .join("rocket-controls-symlink");
        let symlink_src_dir = symlink_dir.join("src/meters/fuel-gauge");
        fs::create_dir_all(&src_dir)?;
        init_repo(&repo_dir).unwrap();
        symlink(&repo_dir, &symlink_dir)?;

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                // Don't truncate the path at all.
                truncation_length = 5
                truncate_to_repo = false
                fish_style_pwd_dir_length = 1
            })
            .path(symlink_src_dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint(format!(
                "{}/above-repo/rocket-controls-symlink/src/meters/fuel-gauge",
                to_fish_style(1, tmp_dir.path().to_slash_lossy(), "")
            ))
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    #[ignore]
    fn fish_path_directory_in_symlinked_git_repo_truncate_to_repo_true() -> io::Result<()> {
        let (tmp_dir, _) = make_known_tempdir(Path::new("/tmp"))?;
        let repo_dir = tmp_dir.path().join("above-repo").join("rocket-controls");
        let src_dir = repo_dir.join("src/meters/fuel-gauge");
        let symlink_dir = tmp_dir
            .path()
            .join("above-repo")
            .join("rocket-controls-symlink");
        let symlink_src_dir = symlink_dir.join("src/meters/fuel-gauge");
        fs::create_dir_all(&src_dir)?;
        init_repo(&repo_dir).unwrap();
        symlink(&repo_dir, &symlink_dir)?;

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                // `truncate_to_repo = true` should display the truncated path
                truncation_length = 5
                truncate_to_repo = true
                fish_style_pwd_dir_length = 1
            })
            .path(symlink_src_dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint(format!(
                "{}/rocket-controls-symlink/src/meters/fuel-gauge",
                to_fish_style(1, tmp_dir.path().join("above-repo").to_slash_lossy(), "")
            ))
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    #[ignore]
    fn directory_in_symlinked_git_repo_truncate_to_repo_true() -> io::Result<()> {
        let (tmp_dir, _) = make_known_tempdir(Path::new("/tmp"))?;
        let repo_dir = tmp_dir.path().join("above-repo").join("rocket-controls");
        let src_dir = repo_dir.join("src/meters/fuel-gauge");
        let symlink_dir = tmp_dir
            .path()
            .join("above-repo")
            .join("rocket-controls-symlink");
        let symlink_src_dir = symlink_dir.join("src/meters/fuel-gauge");
        fs::create_dir_all(&src_dir)?;
        init_repo(&repo_dir).unwrap();
        symlink(&repo_dir, &symlink_dir)?;

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                // `truncate_to_repo = true` should display the truncated path
                truncation_length = 5
                truncate_to_repo = true
            })
            .path(symlink_src_dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan
                .bold()
                .paint("rocket-controls-symlink/src/meters/fuel-gauge")
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    #[ignore]
    fn symlinked_directory_in_git_repo() -> io::Result<()> {
        let (tmp_dir, _) = make_known_tempdir(Path::new("/tmp"))?;
        let repo_dir = tmp_dir.path().join("rocket-controls");
        let dir = repo_dir.join("src");
        fs::create_dir_all(&dir)?;
        init_repo(&repo_dir).unwrap();
        symlink(&dir, repo_dir.join("src/loop"))?;

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                // `truncate_to_repo = true` should display the truncated path
                truncation_length = 5
                truncate_to_repo = true
            })
            .path(repo_dir.join("src/loop/loop"))
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint("rocket-controls/src/loop/loop")
        ));

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    fn truncation_symbol_truncated_root() {
        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                truncation_length = 3
                truncation_symbol = "…/"
            })
            .path(Path::new("/a/four/element/path"))
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint("…/four/element/path")
        ));
        assert_eq!(expected, actual);
    }

    #[test]
    fn truncation_symbol_not_truncated_root() {
        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                truncation_length = 4
                truncation_symbol = "…/"
            })
            .path(Path::new("/a/four/element/path"))
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint("/a/four/element/path")
        ));
        assert_eq!(expected, actual);
    }

    #[test]
    fn truncation_symbol_truncated_home() -> io::Result<()> {
        let (tmp_dir, name) = make_known_tempdir(home_dir().unwrap().as_path())?;
        let dir = tmp_dir.path().join("a/subpath");
        fs::create_dir_all(&dir)?;

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                truncation_length = 3
                truncation_symbol = "…/"
            })
            .path(dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint(format!("…/{}/a/subpath", name))
        ));
        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    fn truncation_symbol_not_truncated_home() -> io::Result<()> {
        let (tmp_dir, name) = make_known_tempdir(home_dir().unwrap().as_path())?;
        let dir = tmp_dir.path().join("a/subpath");
        fs::create_dir_all(&dir)?;

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                truncate_to_repo = false // Necessary if homedir is a git repo
                truncation_length = 4
                truncation_symbol = "…/"
            })
            .path(dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint(format!("~/{}/a/subpath", name))
        ));
        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    fn truncation_symbol_truncated_in_repo() -> io::Result<()> {
        let (tmp_dir, _) = make_known_tempdir(Path::new("/tmp"))?;
        let repo_dir = tmp_dir.path().join("above").join("repo");
        let dir = repo_dir.join("src/sub/path");
        fs::create_dir_all(&dir)?;
        init_repo(&repo_dir).unwrap();

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                truncation_length = 3
                truncation_symbol = "…/"
            })
            .path(dir)
            .collect();
        let expected = Some(format!("{} ", Color::Cyan.bold().paint("…/src/sub/path")));
        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    fn truncation_symbol_not_truncated_in_repo() -> io::Result<()> {
        let (tmp_dir, _) = make_known_tempdir(Path::new("/tmp"))?;
        let repo_dir = tmp_dir.path().join("above").join("repo");
        let dir = repo_dir.join("src/sub/path");
        fs::create_dir_all(&dir)?;
        init_repo(&repo_dir).unwrap();

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                truncation_length = 5
                truncation_symbol = "…/"
                truncate_to_repo = true
            })
            .path(dir)
            .collect();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint("…/repo/src/sub/path")
        ));
        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn truncation_symbol_windows_root_not_truncated() {
        let dir = Path::new("C:\\temp");
        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                truncation_length = 2
                truncation_symbol = "…/"
            })
            .path(dir)
            .collect();
        let expected = Some(format!("{} ", Color::Cyan.bold().paint("C:/temp")));
        assert_eq!(expected, actual);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn truncation_symbol_windows_root_truncated() {
        let dir = Path::new("C:\\temp");
        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                truncation_length = 1
                truncation_symbol = "…/"
            })
            .path(dir)
            .collect();
        let expected = Some(format!("{} ", Color::Cyan.bold().paint("…/temp")));
        assert_eq!(expected, actual);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn truncation_symbol_windows_root_truncated_backslash() {
        let dir = Path::new("C:\\temp");
        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                truncation_length = 1
                truncation_symbol = r"…\"
            })
            .path(dir)
            .collect();
        let expected = Some(format!("{} ", Color::Cyan.bold().paint("…\\temp")));
        assert_eq!(expected, actual);
    }

    #[test]
    fn use_logical_path_true_should_render_logical_dir_path() -> io::Result<()> {
        let tmp_dir = TempDir::new()?;
        let path = tmp_dir.path().join("src/meters/fuel-gauge");
        fs::create_dir_all(&path)?;
        let logical_path = "Logical:/fuel-gauge";

        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint("Logical:/fuel-gauge")
        ));

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                use_logical_path = true
                truncation_length = 3
            })
            .path(path)
            .logical_path(logical_path)
            .collect();

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    fn use_logical_path_false_should_render_current_dir_path() -> io::Result<()> {
        let tmp_dir = TempDir::new()?;
        let path = tmp_dir.path().join("src/meters/fuel-gauge");
        fs::create_dir_all(&path)?;
        let logical_path = "Logical:/fuel-gauge";

        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint("src/meters/fuel-gauge")
        ));

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                use_logical_path = false
                truncation_length = 3
            })
            .path(path)
            .logical_path(logical_path) // logical_path should be ignored
            .collect();

        assert_eq!(expected, actual);
        tmp_dir.close()
    }

    #[test]
    #[cfg(windows)]
    fn windows_trims_extended_path_prefix() {
        // Under Windows, path canonicalization returns the paths using extended-path prefixes `\\?\`
        // We expect this prefix to be trimmed before being rendered.
        let sys32_path = Path::new(r"\\?\C:\Windows\System32");

        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint("C:/Windows/System32")
        ));

        // Note: We have disable the read_only settings here due to false positives when running
        // the tests on Windows as a non-admin.
        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                use_logical_path = false
                truncation_length = 0
                read_only = ""
                read_only_style = ""
            })
            .path(sys32_path)
            .collect();

        assert_eq!(expected, actual);
    }

    #[test]
    #[cfg(windows)]
    fn windows_trims_extended_unc_path_prefix() {
        // Under Windows, path canonicalization returns UNC paths using extended-path prefixes `\\?\UNC\`
        // We expect this prefix to be trimmed before being rendered.
        let unc_path = Path::new(r"\\?\UNC\server\share\a\b\c");

        // NOTE: path-slash doesn't convert slashes which are part of path prefixes under Windows,
        // which is why the first part of this string still includes backslashes
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint(r"\\server\share/a/b/c")
        ));

        let actual = ModuleRenderer::new("directory")
            .config(toml::toml! {
                [directory]
                use_logical_path = false
                truncation_length = 0
            })
            .path(unc_path)
            .collect();

        assert_eq!(expected, actual);
    }

    // sample for invalid unicode from https://doc.rust-lang.org/std/ffi/struct.OsStr.html#method.to_string_lossy
    #[cfg(any(unix, target_os = "redox"))]
    fn invalid_path() -> PathBuf {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        // Here, the values 0x66 and 0x6f correspond to 'f' and 'o'
        // respectively. The value 0x80 is a lone continuation byte, invalid
        // in a UTF-8 sequence.
        let source = [0x66, 0x6f, 0x80, 0x6f];
        let os_str = OsStr::from_bytes(&source[..]);

        PathBuf::from(os_str)
    }

    #[cfg(windows)]
    fn invalid_path() -> PathBuf {
        use std::ffi::OsString;
        use std::os::windows::prelude::*;

        // Here the values 0x0066 and 0x006f correspond to 'f' and 'o'
        // respectively. The value 0xD800 is a lone surrogate half, invalid
        // in a UTF-16 sequence.
        let source = [0x0066, 0x006f, 0xD800, 0x006f];
        let os_string = OsString::from_wide(&source[..]);

        PathBuf::from(os_string)
    }

    #[test]
    #[cfg(any(unix, windows, target_os = "redox"))]
    fn invalid_unicode() {
        let path = invalid_path();
        let expected = Some(format!(
            "{} ",
            Color::Cyan.bold().paint(path.to_string_lossy())
        ));

        let actual = ModuleRenderer::new("directory").path(path).collect();

        assert_eq!(expected, actual);
    }
}
