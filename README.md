# Annotation tool for images

This tool helps to annotate image-segments with labels. It uses the SAM (Facebook Segment anything model) to rapidly generate the masks.
It can be used offline as well as in the browser eventually.

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