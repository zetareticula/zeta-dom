// Copyright 2016 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rusqlite;
use rusqlite::TransactionBehavior;

use vulkano::device::{Device, Queue};
use vulkano::pipeline::{GraphicsPipeline, viewport::Viewport};
use vulkano::buffer::{CpuAccessibleBuffer};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage};
use vulkano::framebuffer::{Framebuffer, Subpass, RenderPass, FramebufferAbstract};
use vulkano::image::{SwapchainImage, ImageUsage};
use vulkano::swapchain::{Swapchain, Surface, PresentMode, SwapchainCreationError};
use vulkano::sync::{self, GpuFuture};
use vulkano::instance::{Instance, PhysicalDevice};
use vulkano::device::DeviceExtensions;
use vulkano::pipeline::shader::ShaderModule;
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;

pub struct VulkanoRenderer {
    device: Arc<Device>,
    queue: Arc<Queue>,
    pipeline: Arc<GraphicsPipeline>,
    swapchain: Arc<Swapchain<Window>>,
    framebuffers: Vec<Arc<dyn FramebufferAbstract + Send + Sync>>,
    render_pass: Arc<RenderPass>,
    metadata: Mutex<Metadata>, // Lock to manage concurrent access
}

impl VulkanoRenderer {
    // Constructor for the renderer
    pub fn new(device: Arc<Device>, queue: Arc<Queue>, pipeline: Arc<GraphicsPipeline>,
               swapchain: Arc<Swapchain<Window>>, framebuffers: Vec<Arc<dyn FramebufferAbstract + Send + Sync>>,
               render_pass: Arc<RenderPass>, metadata: Metadata) -> Self {
        Self {
            device,
            queue,
            pipeline,
            swapchain,
            framebuffers,
            render_pass,
            metadata: Mutex::new(metadata),
        }
    }

    // Load vertex data from the database
    pub fn load_vertex_data(&self, db_path: &str) {
        let partitioned_data = shader_partition_compressor::partition_data(db_path);
        self.apply_partitions(partitioned_data);
    }

    // Applies partitioned shader data to the vertex pipeline
    fn apply_partitions(&self, data: PartitionedData) {
        for block in data.blocks {
            self.apply_shader_block(block);
        }
    }

    // Applies a single block of shader instructions
    fn apply_shader_block(&self, block: ShaderBlock) {
        let (vertex_transform, material_properties) = (block.vertex_transform, block.material_properties);

        // Allocate buffers for vertex data and material properties
        let vertex_buffer = CpuAccessibleBuffer::from_iter(
            self.device.clone(),
            vulkano::buffer::BufferUsage::all(),
            false,
            vertex_transform.iter().cloned()
        ).expect("failed to create buffer");

        let material_buffer = CpuAccessibleBuffer::from_iter(
            self.device.clone(),
            vulkano::buffer::BufferUsage::all(),
            false,
            material_properties.iter().cloned()
        ).expect("failed to create material buffer");

        // Create the command buffer to execute the drawing commands
        let mut builder = AutoCommandBufferBuilder::primary(
            self.device.clone(),
            self.queue.family(),
            CommandBufferUsage::OneTimeSubmit,
        ).unwrap();

        // Bind vertex data and material properties to the shader pipeline
        builder
            .bind_pipeline_graphics(self.pipeline.clone())
            .bind_vertex_buffers(0, vertex_buffer.clone())
            .draw(self.pipeline.clone(), &self.framebuffers[0])
            .unwrap();
        
        let command_buffer = builder.build().unwrap();

        // Execute the command buffer on the GPU
        let future = sync::now(self.device.clone())
            .then_execute(self.queue.clone(), command_buffer)
            .unwrap()
            .then_swapchain_present(self.queue.clone(), self.swapchain.clone(), 0)
            .then_signal_fence_and_flush()
            .unwrap();
        
        future.wait(None).unwrap();
    }

    // Main rendering loop
    pub fn render_loop(&mut self) {
        // Begin the rendering loop
        loop {
            // Handle swapchain recreation errors
            match self.render_frame() {
                Ok(_) => {}
                Err(SwapchainCreationError::OutOfDate) => {
                    self.recreate_swapchain();
                }
                Err(e) => println!("Render error: {:?}", e),
            }
        }
    }

    // Renders a single frame
    fn render_frame(&mut self) -> Result<(), SwapchainCreationError> {
        // Get the next image from the swapchain
        let (image_num, suboptimal, acquire_future) = vulkano::swapchain::acquire_next_image(self.swapchain.clone(), None).unwrap();

        // Render the frame
        let command_buffer = self.build_command_buffer(image_num)?;
        let future = acquire_future
            .then_execute(self.queue.clone(), command_buffer)?
            .then_swapchain_present(self.queue.clone(), self.swapchain.clone(), image_num)
            .then_signal_fence_and_flush();

        future.unwrap().wait(None).unwrap();

        if suboptimal {
            return Err(SwapchainCreationError::Suboptimal);
        }

        Ok(())
    }

    // Builds the command buffer for rendering
    fn build_command_buffer(&self, image_num: usize) -> Result<Arc<vulkano::command_buffer::PrimaryAutoCommandBuffer>, SwapchainCreationError> {
        let framebuffer = self.framebuffers[image_num].clone();
        let mut builder = AutoCommandBufferBuilder::primary(
            self.device.clone(),
            self.queue.family(),
            CommandBufferUsage::OneTimeSubmit,
        ).unwrap();

        builder
            .begin_render_pass(framebuffer.clone(), false, vec![[0.0, 0.0, 0.0, 1.0].into()])
            .unwrap()
            .draw(self.pipeline.clone(), &self.framebuffers[0])
            .unwrap()
            .end_render_pass()
            .unwrap();

        Ok(builder.build().unwrap())
    }

    // Handles swapchain recreation (in case of resizing or updating)
    fn recreate_swapchain(&mut self) {
        let (new_swapchain, new_images) = self.swapchain.recreate().unwrap();
        self.swapchain = new_swapchain;
        self.framebuffers = self.create_framebuffers(new_images);
    }

    // Helper function to create framebuffers for new swapchain images
    fn create_framebuffers(&self, images: Vec<Arc<SwapchainImage<Window>>>) -> Vec<Arc<dyn FramebufferAbstract + Send + Sync>> {
        images.into_iter().map(|image| {
            Arc::new(
                Framebuffer::start(self.render_pass.clone())
                    .add(image.clone())
                    .unwrap()
                    .build()
                    .unwrap()
            ) as Arc<dyn FramebufferAbstract + Send + Sync>
        }).collect::<Vec<_>>()
    }

    // Additional methods for managing shader data and database interactions can be added here
}