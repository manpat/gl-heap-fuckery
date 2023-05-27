



## ideas
special case compute dispatches for image processing
- read GL_COMPUTE_WORK_GROUP_SIZE to determine how many groups to dispatch to cover an image

infer element count for indexed draws



## goals
- single global pipeline containing stages that can be rendered into
- stages describe framebuffer layout and optionally default bindings





## render queue/pipeline stages/render pass
provides default bindings for multiple commands
	normally rendertargets, but can be buffers/images

determines execution order
	separate queues of commands


### Process
At beginning of pass, try get an existing framebuffer with the given attachments
	- if none exists already, create one

If any of the attachments have been recreated (due to resize, etc), resolve and reattach them.

Bind frame buffer, and for each command in renderpass:
	for each active binding in bound pipeline:
		- lookup in command bindings and resolve if found
		- otherwise lookup in render pass bindings and resolve if found
		- otherwise lookup in global bindings and resolve if found
		- otherwise error

		ensure no resources bound are also bound as attachments to the framebuffer


	for each resolved resource:
		bind

		insert memory barrier if resource previously written to
		mark resource as dirty if binding is write or read/write


	dispatch command








# Layers
## device layer
- raw resource management
	- retained mode, manual create/destroy

- manual state binding and draw/compute dispatches

- tracks and handles barrier insertion

- queries and introspection


## next layer
- manages upload heap and transient resources
- knows about render passes and collects/executes command lists
- manages freeing of unused resources


