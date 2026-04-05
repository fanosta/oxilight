use std::convert::TryFrom;

use serde::Deserialize;

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LightStateEnum {
    Off,
    On,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct LightMessage {
    pub brightness: u8,
    pub state: LightStateEnum,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonEvent {
    Press,
    Hold,
    PressRelease,
    HoldRelease,
    Release, // Hue Button does not differentiate...
}

#[derive(Debug, PartialEq)]
pub enum HueDimmerButton {
    On,
    Off,
    Up,
    Down
}

#[derive(Debug, PartialEq)]
pub enum HueTapDialButton {
    Button1,
    Button2,
    Button3,
    Button4,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(try_from = "&str")]
pub struct HueButtonAction {
    pub event: ButtonEvent
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(try_from = "&str")]
pub struct HueDimmerAction {
    pub button: HueDimmerButton,
    pub event: ButtonEvent
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(try_from = "&str")]
pub struct HueTapDialButtonAction {
    pub button: HueTapDialButton,
    pub event: ButtonEvent
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HueTapDialRotationDirection {
    Left,
    Right,
}
#[derive(Debug, PartialEq, Deserialize)]

#[serde(rename_all = "snake_case")]
pub enum HueTapDialRotationType {
    Step,
    Rotate,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct HueButtonMessage {
    pub action: HueButtonAction,
} 

#[derive(Debug, PartialEq, Deserialize)]
pub struct HueDimmerMessage {
    pub action: HueDimmerAction,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum HueTapDialMessage  {
    ButtonMessage {
        action: HueTapDialButtonAction,
    },
    RotateMessage {
        action_direction: HueTapDialRotationDirection,
        action_type: HueTapDialRotationType,
        action_time: u8,
    },
}


impl TryFrom<&str> for ButtonEvent {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "press" => Ok(ButtonEvent::Press),
            "hold" => Ok(ButtonEvent::Hold),
            "press_release" => Ok(ButtonEvent::PressRelease),
            "hold_release" => Ok(ButtonEvent::HoldRelease),
            "release" => Ok(ButtonEvent::Release),
            _ => Err(format!("unknown button event: {s:?}")),
        }
    }
}

impl TryFrom<&str> for HueButtonAction {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Ok(HueButtonAction {
            event: ButtonEvent::try_from(s)?,
        })
    }
}

impl TryFrom<&str> for HueDimmerAction {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let (button_str, event_str) = s.split_once("_").ok_or_else(|| format!("invalid action: {s:?}"))?;

        let button = match button_str {
            "on" => HueDimmerButton::On,
            "off" => HueDimmerButton::Off,
            "up" => HueDimmerButton::Up,
            "down" => HueDimmerButton::Down,
            _ => { return Err(format!("invalid button: {button_str:?}")); },
        };

        Ok(HueDimmerAction {
            button: button,
            event: ButtonEvent::try_from(event_str)?,
        })
    }
}

fn split_at_second_underscore(s: &str) -> Option<(&str, &str)> {
    let mut count = 0;

    for (i, c) in s.char_indices() {
        if c == '_' {
            count += 1;
            if count == 2 {
                return Some((&s[..i], &s[i + 1..]));
            }
        }
    }

    None
}

impl TryFrom<&str> for HueTapDialButtonAction {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let (button_str, event_str) = split_at_second_underscore(s).ok_or_else(|| format!("invalid action: {s:?}"))?;
        // println!("button str: {button_str}, event str: {event_str}");

        let button = match button_str {
            "button_1" => HueTapDialButton::Button1,
            "button_2" => HueTapDialButton::Button2,
            "button_3" => HueTapDialButton::Button3,
            "button_4" => HueTapDialButton::Button4,
            _ => { return Err(format!("invalid button: {button_str:?}")); },
        };

        Ok(HueTapDialButtonAction {
            button: button,
            event: ButtonEvent::try_from(event_str)?,
        })
    }
}
