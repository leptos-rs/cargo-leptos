//! leptos 0.9 serves the pkg dir at one URL and checks this when its options
//! are built. cargo-leptos applies the same rule to 0.9 apps, to report a wrong
//! layout early and to link the CSS at the URL that the app serves it at.

use super::ProjectConfig;
use crate::internal_prelude::*;
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use cargo_metadata::Metadata;
use semver::Version;

/// Whether the app uses leptos 0.9 or newer, which has `site-root-url` and
/// checks the site layout. Apps on older leptos keep the previous behavior.
pub fn uses_site_root_url(metadata: &Metadata, lib: &str, bin: &str) -> bool {
    [lib, bin]
        .into_iter()
        .find_map(|package| leptos_version(metadata, package))
        .is_some_and(is_0_9_or_newer)
}

fn is_0_9_or_newer(version: &Version) -> bool {
    version.major > 0 || version.minor >= 9
}

fn leptos_version<'a>(metadata: &'a Metadata, package: &str) -> Option<&'a Version> {
    let package = metadata
        .workspace_packages()
        .into_iter()
        .find(|p| *p.name == package)?;
    let node = metadata
        .resolve
        .as_ref()?
        .nodes
        .iter()
        .find(|n| n.id == package.id)?;
    node.deps.iter().find_map(|dep| {
        metadata
            .packages
            .iter()
            .find(|p| p.id == dep.pkg && *p.name == "leptos")
            .map(|p| &p.version)
    })
}

/// Returns the URL path that leptos 0.9 serves the pkg dir at, or the error
/// that leptos reports for the layout. Relative paths start at `base`.
pub fn pkg_url(config: &ProjectConfig, base: &Utf8Path) -> Result<String> {
    let root_url = config
        .site_root_url
        .as_deref()
        .unwrap_or("/")
        .trim_matches('/');
    let found = config
        .site_pkg_url
        .as_ref()
        .map(|url| url.as_str().trim_matches('/'))
        .filter(|url| !url.is_empty());
    let pkg_dir = &config.site_pkg_dir;
    let url = match (found, place(&config.site_root, pkg_dir, base)?) {
        (Some(url), _) => url.to_string(),
        (None, Some(place)) if root_url.is_empty() => place,
        (None, Some(place)) => format!("{root_url}/{place}"),
        (None, None) => {
            return Err(eyre!(
                "site-pkg-dir `{pkg_dir}` is outside site-root `{}`, so site-pkg-url must be set",
                config.site_root
            ))
        }
    };
    if url == root_url {
        bail!("site-pkg-url `{url}` must differ from site-root-url");
    }
    Ok(url)
}

/// Returns the path of the pkg dir inside the site root, separated by `/`,
/// or `None` if it is outside the site root.
fn place(site_root: &Utf8Path, pkg_dir: &Utf8Path, base: &Utf8Path) -> Result<Option<String>> {
    // Join both to `base`, so that relative paths, and on Windows paths with a
    // root but no drive, compare the same way as absolute paths.
    let root = normalize(&base.join(site_root));
    let pkg = normalize(&base.join(site_root).join(pkg_dir));
    if root.starts_with(&pkg) {
        bail!("site-pkg-dir `{pkg_dir}` must not be site-root `{site_root}` or contain it");
    }
    Ok(pkg.strip_prefix(&root).ok().map(|place| {
        place
            .components()
            .map(|c| c.as_str())
            .collect::<Vec<_>>()
            .join("/")
    }))
}

/// Removes `.` segments and resolves `..` segments without reading the disk.
fn normalize(path: &Utf8Path) -> Utf8PathBuf {
    let mut normalized = Utf8PathBuf::new();
    for component in path.components() {
        match component {
            Utf8Component::CurDir => {}
            Utf8Component::ParentDir
                if matches!(
                    normalized.components().next_back(),
                    Some(Utf8Component::Normal(_))
                ) =>
            {
                normalized.pop();
            }
            Utf8Component::ParentDir if normalized.has_root() => {}
            component => normalized.push(component.as_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(root: &str, root_url: &str, dir: &str, url: Option<&str>) -> Result<String> {
        let mut config: ProjectConfig = serde_json::from_str("{}").unwrap();
        config.site_root = root.into();
        config.site_root_url = Some(root_url.into());
        config.site_pkg_dir = dir.into();
        config.site_pkg_url = url.map(Into::into);
        pkg_url(&config, Utf8Path::new("/work"))
    }

    #[test]
    fn version_boundary() {
        for (version, expected) in [
            ("0.8.22", false),
            ("0.9.0-rc0", true),
            ("0.9.0", true),
            ("1.0.0", true),
        ] {
            assert_eq!(is_0_9_or_newer(&Version::parse(version).unwrap()), expected);
        }
    }

    #[test]
    fn site_layout() {
        for (root, root_url, dir, given, expected) in [
            ("target/site", "/", "pkg", None, "pkg"),
            ("target/site", "/static/", "pkg", None, "static/pkg"),
            ("target/site", "/", "pkg", Some("/pkg/"), "pkg"),
            ("target/site", "/", "pkg", Some(""), "pkg"),
            ("target/site", "/", "pkg", Some("assets"), "assets"),
            ("target/site", "/static", "pkg", Some("pkg"), "pkg"),
            ("target/site", "/", "./a/../pkg", None, "pkg"),
            ("/srv/site", "/", "/srv/site/a/pkg", None, "a/pkg"),
            ("target/site", "/", "../pkg", Some("assets"), "assets"),
            (".", "/", "/work/pkg", None, "pkg"),
        ] {
            assert_eq!(
                url(root, root_url, dir, given).unwrap(),
                expected,
                "{root} {root_url} {dir} {given:?}"
            );
        }

        for (root, root_url, dir, given, error) in [
            (
                "target/site",
                "/",
                "../pkg",
                None,
                "site-pkg-url must be set",
            ),
            (
                "target/site",
                "/static",
                "../pkg",
                Some("/static/"),
                "must differ",
            ),
            (
                "target/site",
                "/static",
                "pkg",
                Some("static"),
                "must differ",
            ),
            ("target/site", "/", "", None, "must not be site-root"),
            ("target/site", "/", "..", None, "must not be site-root"),
        ] {
            let found = url(root, root_url, dir, given).unwrap_err().to_string();
            assert!(
                found.contains(error),
                "{root} {root_url} {dir} {given:?}: {found}"
            );
        }
    }
}
