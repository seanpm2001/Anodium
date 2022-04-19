use crate::{state::output::OutputState, State};

use anodium_backend::{
    utils::cursor::PointerElement, NewOutputDescriptor, OutputHandler, OutputId,
};

use smithay::{
    backend::renderer::gles2::{Gles2Renderer, Gles2Texture},
    desktop::space::SurfaceTree,
    wayland::output::{Mode, Output},
};
use smithay_egui::EguiFrame;

smithay::custom_elements! {
    pub CustomElem<=Gles2Renderer>;
    SurfaceTree=SurfaceTree,
    PointerElement=PointerElement,
    EguiFrame=EguiFrame,
}

impl OutputHandler for State {
    fn output_created(&mut self, desc: NewOutputDescriptor) {
        let layout = self.config.output_layout();

        let (output, _) = Output::new(
            &mut self.display.borrow_mut(),
            desc.name.clone(),
            desc.physical_properties,
            None,
        );
        output.set_preferred(desc.prefered_mode);

        output.user_data().insert_if_missing(|| desc.id);

        if let Some(c) = layout.find_output(&desc.name) {
            output.change_current_state(Some(c.mode()), Some(desc.transform), None, None);
            self.backend
                .update_mode(output.user_data().get::<OutputId>().unwrap(), &c.mode());
        } else {
            output.change_current_state(Some(desc.prefered_mode), Some(desc.transform), None, None);
        }

        let mut outputs: Vec<_> = self
            .space
            .outputs()
            .cloned()
            .chain(std::iter::once(output))
            .collect();

        let mut x = 0;

        // Map all configured outputs first
        for desc in layout.iter() {
            if let Some(id) = outputs.iter().position(|o| o.name() == desc.name()) {
                let output = outputs.remove(id);

                let location = (x, 0).into();
                self.space.map_output(&output, 1.0, location);

                output.change_current_state(None, None, None, Some(location));

                x += desc.mode().size.w;
            }
        }

        // Put unconfigured outputs on the end
        for output in outputs.into_iter().rev() {
            let location = (x, 0).into();
            self.space.map_output(&output, 1.0, location);
            output.change_current_state(None, None, None, Some(location));

            x += output.current_mode().unwrap().size.w;
        }
    }

    fn output_mode_updated(&mut self, output_id: &OutputId, mode: Mode) {
        let output = self
            .space
            .outputs()
            .find(|o| o.user_data().get::<OutputId>() == Some(output_id));

        if let Some(output) = output {
            output.change_current_state(Some(mode), None, None, None);
        }
    }

    fn output_render(
        &mut self,
        renderer: &mut Gles2Renderer,
        output_id: &OutputId,
        age: usize,
        pointer_image: Option<&Gles2Texture>,
    ) -> Result<
        Option<Vec<smithay::utils::Rectangle<i32, smithay::utils::Logical>>>,
        smithay::backend::SwapBuffersError,
    > {
        let mut elems: Vec<CustomElem> = Vec::new();

        let location = self
            .seat
            .get_pointer()
            .unwrap()
            .current_location()
            .to_i32_round();

        if let Some(tree) = self.pointer_icon.prepare_dnd_icon(location) {
            elems.push(tree.into());
        }

        if let Some(tree) = self.pointer_icon.prepare_cursor_icon(location) {
            elems.push(tree.into());
        } else if let Some(texture) = pointer_image {
            elems.push(PointerElement::new(texture.clone(), location, false).into());
        }

        let output = self
            .space
            .outputs()
            .find(|o| o.user_data().get::<OutputId>() == Some(output_id))
            .unwrap()
            .clone();

        let output_state = OutputState::from_output(&output);
        let egui = output_state.egui_frame(&output, &self.start_time);
        elems.push(egui.into());

        let render_result = self
            .space
            .render_output(renderer, &output, age, [0.1, 0.1, 0.1, 1.0], &elems)
            .unwrap();

        if render_result.is_some() {
            output_state.fps_tick();
        }

        Ok(render_result)
    }
}
