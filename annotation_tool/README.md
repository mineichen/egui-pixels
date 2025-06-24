# Annotation tool for images

This tool helps to annotate image-segments with labels. It uses the SAM (Facebook Segment anything model) to rapidly generate the masks.
It can be used offline as well as in the browser eventually.

## Setup

mkdir sam
wget https://github.com/AndreyGermanov/sam_onnx_rust/raw/refs/heads/main/vit_t_encoder.onnx -O sam/vit_t_encoder.onnx
wget https://github.com/AndreyGermanov/sam_onnx_rust/raw/refs/heads/main/vit_t_decoder.onnx -O sam/vit_t_decoder.onnx

## Run Web

trunk serve --release --no-default-features

## Run native with SAM

cargo run --release --features sam -- ../path/to/your/images

Features

- Select image from list
- Zoom / pan image
- Load persisted annotations
- Create segments with SAM
- Ctrl+Z and Ctrl+Shift+Z for stepwise redo/undo

In progress

- Persist annotations
- HTTP-Backend and inference via WebGPU (onnxruntime)

Ideas for the future

- List annotations in the UI
- More tools
  - Paint-Brush with different sizes
  - Fill/remove area behind a square
  - Connect existing segments
