use zbus::fdo::Result;

pub async fn generate_files(css: bool, yuck: bool) -> Result<()> {
    let xdg_config_home = std::env::var("XDG_CONFIG_HOME")
        .unwrap_or_else(|_| format!("{}/.config", std::env::var("HOME").unwrap()));
    let css_path = format!("{}/eww/end.scss", xdg_config_home);
    let yuck_path = format!("{}/eww/end.yuck", xdg_config_home);

    if css {
        async_fs::write(&css_path, include_str!("../assets/end.scss"))
            .await
            .map_err(|e| {
                zbus::fdo::Error::Failed(format!("Failed to write to file {}: {}", css_path, e))
            })?;
    }

    if yuck {
        async_fs::write(&yuck_path, include_str!("../assets/end.yuck"))
            .await
            .map_err(|e| {
                zbus::fdo::Error::Failed(format!("Failed to write to file {}: {}", yuck_path, e))
            })?
    }

    Ok(())
}
