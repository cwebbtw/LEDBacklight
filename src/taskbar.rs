//! Read-only shell taskbar visibility; no game/process access or window changes.
use crate::layout::Region;
use windows::{
    Win32::{
        Foundation::{POINT, RECT},
        UI::WindowsAndMessaging::*,
    },
    core::w,
};

pub fn visible_regions(origin: (i32, i32), bounds: Region) -> Vec<Region> {
    let mut result = Vec::new();
    // Inspect only Windows shell taskbar windows. IsWindowVisible alone is
    // insufficient: a fullscreen game can cover a still-visible taskbar HWND.
    unsafe {
        let mut windows = Vec::new();
        if let Ok(hwnd) = FindWindowW(w!("Shell_TrayWnd"), None) {
            windows.push(hwnd);
        }
        let mut after = None;
        for _ in 0..16 {
            let Ok(hwnd) = FindWindowExW(None, after, w!("Shell_SecondaryTrayWnd"), None) else {
                break;
            };
            windows.push(hwnd);
            after = Some(hwnd);
        }
        for hwnd in windows {
            let mut rect = RECT::default();
            if !IsWindowVisible(hwnd).as_bool() || GetWindowRect(hwnd, &mut rect).is_err() {
                continue;
            }
            let exposed = [1, 2, 3].into_iter().any(|i| {
                let wide = rect.right - rect.left >= rect.bottom - rect.top;
                let point = POINT {
                    x: rect.left + (rect.right - rect.left) * if wide { i } else { 2 } / 4,
                    y: rect.top + (rect.bottom - rect.top) * if wide { 2 } else { i } / 4,
                };
                GetAncestor(WindowFromPoint(point), GA_ROOT) == hwnd
            });
            if let Some(region) = clipped(rect, origin, bounds, exposed) {
                result.push(region);
            }
        }
    }
    result
}
pub(crate) fn clipped(
    rect: RECT,
    origin: (i32, i32),
    bounds: Region,
    visible: bool,
) -> Option<Region> {
    if !visible {
        return None;
    }
    let ax = i64::from(origin.0) + bounds.x as i64;
    let ay = i64::from(origin.1) + bounds.y as i64;
    let left = i64::from(rect.left).max(ax);
    let top = i64::from(rect.top).max(ay);
    let right = i64::from(rect.right).min(ax + bounds.width as i64);
    let bottom = i64::from(rect.bottom).min(ay + bounds.height as i64);
    if right <= left || bottom <= top {
        return None;
    }
    Some(Region {
        x: (left - ax) as usize,
        y: (top - ay) as usize,
        width: (right - left) as usize,
        height: (bottom - top) as usize,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visible_taskbar_only_and_monitor_crop_offsets() {
        let rect = RECT {
            left: 0,
            top: 1400,
            right: 3440,
            bottom: 1440,
        };
        let full = Region {
            x: 0,
            y: 0,
            width: 3440,
            height: 1440,
        };
        assert_eq!(
            clipped(rect, (0, 0), full, true),
            Some(Region {
                x: 0,
                y: 1400,
                width: 3440,
                height: 40
            })
        );
        assert_eq!(clipped(rect, (0, 0), full, false), None); // Covered by fullscreen content.
        assert_eq!(
            clipped(
                rect,
                (-1920, 0),
                Region {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080
                },
                true
            ),
            None
        );
        assert_eq!(
            clipped(
                rect,
                (0, 0),
                Region {
                    x: 100,
                    y: 20,
                    width: 3200,
                    height: 1390
                },
                true
            ),
            Some(Region {
                x: 0,
                y: 1380,
                width: 3200,
                height: 10
            })
        );
        let hidden = RECT {
            left: 0,
            top: 1440,
            right: 3440,
            bottom: 1480,
        };
        assert_eq!(clipped(hidden, (0, 0), full, true), None);
    }
}
