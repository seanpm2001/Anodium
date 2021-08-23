use std::cell::RefCell;

use smithay::{
    reexports::wayland_server::protocol::wl_surface,
    utils::{Logical, Point, Rectangle},
    wayland::{
        compositor::{with_states, with_surface_tree_downward, SubsurfaceCachedState, TraversalAction},
        shell::wlr_layer::{self, Anchor, ExclusiveZone, LayerSurfaceCachedState},
    },
};

use crate::shell::SurfaceData;

#[derive(Default, Debug)]
pub struct LayerExclusiveZone {
    pub top: u32,
    pub bottom: u32,
    pub left: u32,
    pub right: u32,
}

#[derive(Debug)]
pub struct LayerSurface {
    pub surface: wlr_layer::LayerSurface,
    pub location: Point<i32, Logical>,
    pub bbox: Rectangle<i32, Logical>,
    pub layer: wlr_layer::Layer,
}

impl LayerSurface {
    /// Finds the topmost surface under this point if any and returns it together with the location of this
    /// surface.
    fn matching(&self, point: Point<f64, Logical>) -> Option<(wl_surface::WlSurface, Point<i32, Logical>)> {
        if !self.bbox.to_f64().contains(point) {
            return None;
        }
        // need to check more carefully
        let found = RefCell::new(None);
        if let Some(wl_surface) = self.surface.get_surface() {
            with_surface_tree_downward(
                wl_surface,
                self.location,
                |wl_surface, states, location| {
                    let mut location = *location;
                    let data = states.data_map.get::<RefCell<SurfaceData>>();

                    if states.role == Some("subsurface") {
                        let current = states.cached_state.current::<SubsurfaceCachedState>();
                        location += current.location;
                    }

                    let contains_the_point = data
                        .map(|data| {
                            data.borrow()
                                .contains_point(&*states.cached_state.current(), point - location.to_f64())
                        })
                        .unwrap_or(false);
                    if contains_the_point {
                        *found.borrow_mut() = Some((wl_surface.clone(), location));
                    }

                    TraversalAction::DoChildren(location)
                },
                |_, _, _| {},
                |_, _, _| {
                    // only continue if the point is not found
                    found.borrow().is_none()
                },
            );
        }
        found.into_inner()
    }

    fn self_update(&mut self) {
        let mut bounding_box = Rectangle::from_loc_and_size(self.location, (0, 0));
        if let Some(wl_surface) = self.surface.get_surface() {
            with_surface_tree_downward(
                wl_surface,
                self.location,
                |_, states, &loc| {
                    let mut loc = loc;
                    let data = states.data_map.get::<RefCell<SurfaceData>>();

                    if let Some(size) = data.and_then(|d| d.borrow().size()) {
                        if states.role == Some("subsurface") {
                            let current = states.cached_state.current::<SubsurfaceCachedState>();
                            loc += current.location;
                        }

                        // Update the bounding box.
                        bounding_box = bounding_box.merge(Rectangle::from_loc_and_size(loc, size));

                        TraversalAction::DoChildren(loc)
                    } else {
                        // If the parent surface is unmapped, then the child surfaces are hidden as
                        // well, no need to consider them here.
                        TraversalAction::SkipChildren
                    }
                },
                |_, _, _| {},
                |_, _, _| true,
            );
        }
        self.bbox = bounding_box;

        if let Some(surface) = self.surface.get_surface() {
            self.layer = with_states(surface, |states| {
                let current = states.cached_state.current::<LayerSurfaceCachedState>();
                current.layer
            })
            .unwrap();
        }
    }

    /// Sends the frame callback to all the subsurfaces in this
    /// window that requested it
    fn send_frame(&self, time: u32) {
        if let Some(wl_surface) = self.surface.get_surface() {
            with_surface_tree_downward(
                wl_surface,
                (),
                |_, _, &()| TraversalAction::DoChildren(()),
                |_, states, &()| {
                    // the surface may not have any user_data if it is a subsurface and has not
                    // yet been commited
                    SurfaceData::send_frame(&mut *states.cached_state.current(), time)
                },
                |_, _, &()| true,
            );
        }
    }
}

#[derive(Default, Debug)]
pub struct LayerMap {
    surfaces: Vec<LayerSurface>,
    exclusive_zone: LayerExclusiveZone,
}

impl LayerMap {
    pub fn exclusive_zone(&self) -> &LayerExclusiveZone {
        &self.exclusive_zone
    }
}

impl LayerMap {
    pub fn insert(&mut self, surface: wlr_layer::LayerSurface, layer: wlr_layer::Layer) {
        let mut layer = LayerSurface {
            location: Default::default(),
            bbox: Rectangle::default(),
            surface,
            layer,
        };
        layer.self_update();
        self.surfaces.insert(0, layer);
    }

    pub fn get_surface_under(
        &self,
        layer: &wlr_layer::Layer,
        point: Point<f64, Logical>,
    ) -> Option<(wl_surface::WlSurface, Point<i32, Logical>)> {
        for l in self.surfaces.iter().filter(|s| &s.layer == layer) {
            if let Some(surface) = l.matching(point) {
                return Some(surface);
            }
        }
        None
    }

    pub fn with_layers_from_bottom_to_top<Func>(&self, layer: &wlr_layer::Layer, mut f: Func)
    where
        Func: FnMut(&LayerSurface),
    {
        for l in self.surfaces.iter().filter(|s| &s.layer == layer).rev() {
            f(l)
        }
    }

    pub fn refresh(&mut self) {
        self.surfaces.retain(|l| l.surface.alive());

        for l in self.surfaces.iter_mut() {
            l.self_update();
        }
    }

    #[allow(dead_code)]
    /// Finds the layer corresponding to the given `WlSurface`.
    pub fn find(&self, surface: &wl_surface::WlSurface) -> Option<&LayerSurface> {
        self.surfaces.iter().find_map(|l| {
            if l.surface
                .get_surface()
                .map(|s| s.as_ref().equals(surface.as_ref()))
                .unwrap_or(false)
            {
                Some(l)
            } else {
                None
            }
        })
    }

    pub fn arange(&mut self, output_rect: Rectangle<i32, Logical>) {
        self.exclusive_zone = Default::default();

        for layer in self.surfaces.iter_mut() {
            let surface = if let Some(surface) = layer.surface.get_surface() {
                surface
            } else {
                continue;
            };

            let data = with_states(surface, |states| {
                *states.cached_state.current::<LayerSurfaceCachedState>()
            })
            .unwrap();

            let x = if data.size.w == 0 || data.anchor.contains(Anchor::LEFT) {
                output_rect.loc.x
            } else if data.anchor.contains(Anchor::RIGHT) {
                output_rect.loc.x + (output_rect.size.w - data.size.w)
            } else {
                output_rect.loc.x + ((output_rect.size.w / 2) - (data.size.w / 2))
            };

            let y = if data.size.h == 0 || data.anchor.contains(Anchor::TOP) {
                output_rect.loc.y
            } else if data.anchor.contains(Anchor::BOTTOM) {
                output_rect.loc.y + (output_rect.size.h - data.size.h)
            } else {
                output_rect.loc.y + ((output_rect.size.h / 2) - (data.size.h / 2))
            };

            let location: Point<i32, Logical> = (x, y).into();

            layer
                .surface
                .with_pending_state(|state| {
                    state.size = Some(output_rect.size);
                })
                .unwrap();

            layer.surface.send_configure();

            layer.location = location;

            if let ExclusiveZone::Exclusive(v) = data.exclusive_zone {
                let anchor = data.anchor;

                // Top
                if anchor == (Anchor::TOP) {
                    self.exclusive_zone.top += v;
                }
                if anchor == (Anchor::TOP | Anchor::LEFT | Anchor::RIGHT) {
                    self.exclusive_zone.top += v;
                }

                // Bottom
                if anchor == (Anchor::BOTTOM) {
                    self.exclusive_zone.bottom += v;
                }
                if anchor == (Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT) {
                    self.exclusive_zone.bottom += v;
                }

                // Left
                if anchor == (Anchor::LEFT) {
                    self.exclusive_zone.left += v;
                }
                if anchor == (Anchor::LEFT | Anchor::BOTTOM | Anchor::TOP) {
                    self.exclusive_zone.left += v;
                }

                // Right
                if anchor == (Anchor::RIGHT) {
                    self.exclusive_zone.right += v;
                }
                if anchor == (Anchor::RIGHT | Anchor::BOTTOM | Anchor::TOP) {
                    self.exclusive_zone.right += v;
                }
            }
        }
    }

    pub fn send_frames(&self, time: u32) {
        for layer in &self.surfaces {
            layer.send_frame(time);
        }
    }
}
