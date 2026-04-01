use openaction::{get_instance, Instance, OpenActionResult};

use crate::icons::{
    apply_muted_stroke, create_blank_button, create_blank_knob, create_knob_display,
    create_unlocked_button_with_icon, get_app_icon_base64,
};

pub fn format_knob_label(name: &str) -> String {
    const MAX_CHARS: usize = 9;
    let chars: Vec<char> = name.chars().collect();

    if chars.len() <= MAX_CHARS {
        return name.to_string();
    }

    for (i, &c) in chars.iter().enumerate() {
        if c == ' ' && i > 0 && i < chars.len() - 1 && i <= MAX_CHARS {
            let first: String = chars.iter().take(i).collect();
            let second: String = chars.iter().skip(i + 1).take(MAX_CHARS).collect();
            return format!("{}\n{}", first, second);
        }
    }

    let first: String = chars.iter().take(MAX_CHARS).collect();
    let second: String = chars.iter().skip(MAX_CHARS).take(MAX_CHARS).collect();
    format!("{}\n{}", first, second)
}

pub fn blank_button_image() -> String {
    create_blank_button()
}

pub fn blank_knob_image() -> String {
    create_blank_knob()
}

pub fn knob_image(name: &str, volume: u32, muted: bool) -> String {
    create_knob_display(name, volume, muted)
}

pub fn unlocked_button_image(icon_b64: &str, is_light: bool) -> String {
    create_unlocked_button_with_icon(icon_b64, is_light)
}

pub async fn button_icon_image(icon_query: &str, muted: bool) -> Option<String> {
    let icon_b64 = get_app_icon_base64(icon_query).await?;
    Some(if muted {
        apply_muted_stroke(&icon_b64)
    } else {
        icon_b64
    })
}

pub async fn set_button_instance_icon(
    instance: &Instance,
    icon_query: &str,
    muted: bool,
) -> OpenActionResult<Option<String>> {
    let icon = button_icon_image(icon_query, muted).await;
    if let Some(ref image) = icon {
        instance.set_image(Some(image.clone()), None).await?;
    }
    Ok(icon)
}

pub async fn update_knob_instances(
    knob_instance_ids: Vec<String>,
    title: &str,
    volume: u32,
    muted: bool,
) {
    let display = knob_image(title, volume, muted);
    let title = format_knob_label(title);

    for knob_instance_id in knob_instance_ids {
        if let Some(knob) = get_instance(knob_instance_id).await {
            knob.set_image(Some(display.clone()), None).await.ok();
            knob.set_title(Some(title.clone()), None).await.ok();
        }
    }
}

pub async fn clear_knob_instances(knob_instance_ids: Vec<String>) {
    for knob_instance_id in knob_instance_ids {
        if let Some(knob) = get_instance(knob_instance_id).await {
            knob.set_image(Some(blank_knob_image()), None).await.ok();
            knob.set_title(None::<String>, None).await.ok();
        }
    }
}

pub async fn update_button_instances(
    button_instance_ids: Vec<String>,
    icon_query: &str,
    muted: bool,
) -> Option<String> {
    let icon = button_icon_image(icon_query, muted).await?;

    for instance_id in button_instance_ids {
        if let Some(instance) = get_instance(instance_id).await {
            instance.set_image(Some(icon.clone()), None).await.ok();
        }
    }

    Some(icon)
}
