# WWDC25–26 APIs for Apple GPU compute and ML

Snapshot: **11 October 2026**; SDK availability rechecked on 13 October 2026 against the macOS 27.0 SDK (26A425) and the macOS 26.5 SDK. macOS 26 corresponds to WWDC25; macOS 27 corresponds to WWDC26. This chapter lists the Metal, Metal Performance Primitives (TensorOps), Core AI and related APIs from those two years that matter for GPU compute and ML inference on Apple silicon, and the limits that decide whether they apply. No performance result is claimed for any API listed here.

## Coverage and evidence

The source census starts with the complete official [WWDC25](https://developer.apple.com/videos/wwdc2025/) and [WWDC26](https://developer.apple.com/videos/wwdc2026/) video catalogs: **122 + 138 = 260 entries**. A recorded topic/title filter selected **66 sessions** covering Metal, ML/AI frameworks, relevant tools and adjacent applications. All 66 transcript and code sections were retrieved; the catalog below distinguishes direct compute relevance from adjacent material. Two M5 technical talks supplement the WWDC material.

This is an inventory of **API families introduced, expanded or demonstrated in those materials**, not an exhaustive diff of every SDK declaration. “Shown in WWDC25” does not automatically mean “introduced in macOS 26.” For example, MLX APIs and residency sets predate some of these talks. In the installed macOS 27.0 SDK (`MTLTensor.h`), Int4/UInt4 tensor types are marked **26.4**, while FP4/FP8, Int2 and auxiliary scale planes are marked **27.0**. Likewise, “shown in WWDC26” does not mean “new in macOS 27”: the MPP cooperative-input reuse overloads, `is_compatible_as_*_input`, `reduce_rows` and `is_iterator_compatible` are already in the macOS 26.5 SDK headers; only the FP4/FP8/Int2 formats and block-scaled (`tensor_blockwise`) operands are absent there.

A compact summary of the census (counts and hashes) is in [wwdc-api-audit.json](evidence/2026-10-11/wwdc-api-audit.json); the raw catalog and transcripts were not retained.

## Important compatibility limits

1. **Native scale planes are not arbitrary quantization metadata.** The documented scales plane uses E8M0, with a 32-element first-dimension block and 1 in the remaining dimensions. Arbitrary per-group scales (for example BF16 per 64 elements) and affine zero points are not interchangeable with it; repeating a 64-group scale twice does not fix the datatype or zero-point mismatch. [Block-factor contract](https://developer.apple.com/documentation/metal/mtltensorauxiliaryplanedescriptor/blockfactors).
2. **Multi-plane tensors cannot use `MTLTensorUsage.machineLearning`.** They support compute/render usage, so a scaled TensorOps path and a machine-learning-encoder path must stay separate. [Descriptor contract](https://developer.apple.com/documentation/metal/mtltensordescriptor/auxiliaryplanes).
3. **Compatible cooperative tensors can feed another matrix operation; arbitrary tensors cannot.** Use `is_compatible_as_left_input` / `is_compatible_as_right_input` before reuse. A changed dtype, shape or layout can require conversion. Private storage can still spill. When both inputs are cooperative tensors, M, N and K must each be 16 or 32 (`static_assert` in `MPPTensorOpsMatMul2dImpl.h`), so a K64 product needs tensor-view operands. [WWDC26/330](https://developer.apple.com/videos/play/wwdc2026/330/).
4. **“Zero-copy view” is not a guarantee of zero runtime conversion.** The SDK lets Core AI views wrap a Metal buffer or IOSurface (`NDArray.MutableRawView(metalBuffer:)`, `(ioSurface:)`) and exposes `NDArrayDescriptor.preferredStrides`. Whether a non-preferred layout or the selected compute unit forces an internal copy is not verified here (the documentation page originally cited for this, `coreai_extended_api_documentation`, returned 404 on 2026-10-13). Include interop and layout conversion in timing.
5. **Compilation speed is different from inference speed.** Archive lookup, asynchronous compilation and AOT can accelerate tuning/startup without changing steady-state GEMM time. Missing archive entries must remain explicit.
6. **API availability is not hardware throughput evidence.** A low-bit operand can be accepted while lowering to unpacking: MPP int8 × int4 widens the nibbles in ALU code and then issues the same int8 MXU instructions as int8 × int8 (AIR of `libTensorOps.rtlib`, `__tensorops_impl_matmul2d_op_run_cooperative_dv_i8_dv_i4_i32_v2`).

The macOS 27.0 SDK ships `CoreAI.framework` with a native `arm64e-apple-macos` Swift interface (`InferenceFunction` is declared in the `CoreAIRuntime` subframework), and a macOS 27 typecheck of `CoreAI.InferenceFunction` passed on 2026-10-11. The retrieved DocC platform list for that symbol omitted macOS; the SDK check is retained rather than treating that metadata omission as lack of support. Runtime model conversion/inference has not been tested here.

## API inventory

Each family links to its session or API contract; rows without a link name the SDK declaration. “SDK verified” means the era comes from the availability annotation in the installed SDK; otherwise it is the WWDC year that presented the API.

### TensorOps

| ID | API / capability | Era | Notes and limits |
| --- | --- | --- | --- |
| T01 | [tensor_inline / tensor_handle; tensor::slice](https://developer.apple.com/videos/play/wwdc2026/330/) | 26 | Strided views over existing memory. Physical packing and transforms stay explicit; strides cannot express every custom packing. |
| T02 | [MTLTensorDescriptor; MTLDevice.makeTensor; MTLTensor.gpuResourceID](https://developer.apple.com/videos/play/wwdc2025/262/) | 26 | Dense/packed tensor resources. A device-owned opaque layout may need a one-time copy. |
| T03 | [matmul2d with dynamic_length_v&lt;int&gt;; execution_simdgroups&lt;N&gt;](https://developer.apple.com/videos/play/wwdc2026/330/) [also 1](https://developer.apple.com/download/files/Metal-Performance-Primitives-Programming-Guide.pdf) | 26 | Dynamic extents and multi-SIMD-group execution scope. With both inputs cooperative, M, N and K must be 16 or 32. |
| T04 | [get_left_input_cooperative_tensor(source); get_right_input_cooperative_tensor(source)](https://developer.apple.com/videos/play/wwdc2026/330/) | shown in WWDC26; already in macOS 26.5 SDK headers | Keeps compatible intermediates in cooperative storage. The zero-argument constructor is not reuse. |
| T05 | [is_compatible_as_left_input / is_compatible_as_right_input](https://developer.apple.com/videos/play/wwdc2026/330/) | shown in WWDC26; already in macOS 26.5 SDK headers | Proves a producer layout can feed the next matrix operation. Reject incompatible layouts; never reinterpret blindly. |
| T06 | [reduce_rows; reduction_operation; map_iterator](https://developer.apple.com/videos/play/wwdc2026/330/) | shown in WWDC26; `reduce_rows` and `is_iterator_compatible` already in macOS 26.5 SDK headers | Reductions (for example attention softmax) in cooperative layout. Reduction order and tails may change output bits. |
| T07 | [MTLTensorDataTypeInt4 / UInt4; int4b_format / uint4b_format](https://developer.apple.com/videos/play/wwdc2026/330/) | 26.4 (SDK verified) | Storage support is not native int4 throughput: int8 × int4 lowers to ALU widening plus int8 MXU instructions (limit 6). |
| T08 | [Int2 / UInt2; MetalFloat4E2M1; MetalFloat8E4M3 / E5M2](https://developer.apple.com/documentation/Metal/MTLTensorDataType) | 27 (SDK verified) | Measure format-specific ceilings before converting a model. Not a drop-in encoding for affine-quantized weights. |
| T09 | [MTLTensorAuxiliaryPlaneDescriptor / Map; tensor_blockwise; MetalFloat8UE8M0](https://developer.apple.com/documentation/Metal/MTLTensorAuxiliaryPlaneDescriptor) | 27 (SDK verified) | Hardware-supported block scales: E8M0 only, block factors 32 then 1. Affine zero points are not representable. |
| T10 | [MTLTensorBufferAttachments; makeTensor(descriptor:attachments:)](<https://developer.apple.com/documentation/Metal/MTLDevice/makeTensor(descriptor:attachments:)>) | 27 (SDK verified) | Binds existing data and scale buffers as separate planes. Every configured plane needs a buffer; alignment and strides are enforced. |
| T11 | [MTLTensor auxiliaryPlanes; per-plane getBytes / replaceSliceOrigin](https://developer.apple.com/documentation/Metal/MTLTensorDescriptor/auxiliaryPlanes) | 27 (SDK verified) | Validate scale and data plane layout independently. Multi-plane tensors cannot use machineLearning usage. |
| T12 | [Explicit K tiling, periodic threadgroup synchronization and cache-aware tile traversal](https://developer.apple.com/videos/play/tech-talks/111432/) | M5 technical talk; strategy rather than a new API | Coordinated multi-SIMD-group K tiles and Morton/Hilbert traversal. Useful only when groups share data; extra barriers or larger live tiles can lose. |

### Compiled networks

| ID | API / capability | Era | Notes and limits |
| --- | --- | --- | --- |
| N01 | [MTL4MachineLearningCommandEncoder; dispatchNetworkWithIntermediatesHeap](https://developer.apple.com/videos/play/wwdc2025/262/) | 26 | Compiler-generated network on the GPU timeline. A separate execution path, not a transparent switch for existing shaders. |
| N02 | [MTL4MachineLearningPipelineDescriptor; setInputDimensions; reflection; intermediatesHeapSize](https://developer.apple.com/documentation/Metal/running-a-machine-learning-model-on-the-gpu-timeline) | 26 | Specialize input dimensions and allocate reusable intermediates. Full tensor signatures and dynamic dimensions must match. |
| N03 | [metal-package-builder; MTLPackage loading as MTLLibrary](https://developer.apple.com/videos/play/wwdc2025/262/) | 26 | Requires a supported model graph; custom quantization is not automatic. |
| N04 | [MTLStageMachineLearning with Metal 4 barriers / fences](https://developer.apple.com/videos/play/wwdc2025/262/) | 26 | Synchronizes mixed compute/ML work. A threadgroup barrier gives no global ordering. |

### Compilation

| ID | API / capability | Era | Notes and limits |
| --- | --- | --- | --- |
| C01 | [MTL4PipelineDataSetSerializer; CaptureDescriptors / CaptureBinaries; serializeAsPipelinesScript](https://developer.apple.com/videos/play/wwdc2025/254/) | 26 | Harvests exact runtime specializations for offline compilation. Record the exact function constants, platform and target. |
| C02 | [metal-tt; MTL4Archive.newComputePipelineStateWithDescriptor](https://developer.apple.com/videos/play/wwdc2025/254/); `MTL4CompilerTaskOptions.lookupArchives` (`MTL4Compiler.h`) | 26 | Reduces repeated compilation and load latency. Offline translation of MPP cooperative-tensor helpers fails with `unsupported deferred-static-alloca-size` ([compiler chapter](compiler.md)); an archive miss must stay visible. |
| C03 | [MTL4Compiler asynchronous tasks / QoS; maximumConcurrentCompilationTaskCount](https://developer.apple.com/videos/play/wwdc2025/254/) | Metal 4 compiler in 26; `maximumConcurrentCompilationTaskCount` since macOS 13.3 | Bounded parallel compilation, kept outside timing windows. Improves iteration/startup, not arithmetic throughput. |

### Profiling

| ID | API / capability | Era | Notes and limits |
| --- | --- | --- | --- |
| P01 | [MTL4CounterHeap; writeTimestamp; queryTimestampFrequency; resolveCounterHeap](https://developer.apple.com/documentation/Metal/MTL4CounterHeap) | 26 | Component timing. Precise timestamp writes can split passes; keep separate from end-to-end timing. |
| P02 | [StateReporter / SRStateReporter; reportTransition; reportVolatileMetadataUpdate](https://developer.apple.com/videos/play/wwdc2026/388/) | 27 (docs/SDK verified) | Labels configuration/workload epochs in long traces. The system throttles frequent transitions; not a per-dispatch event stream. |
| P03 | [MetricManager.metricReports / diagnosticReports; MetricReport state aggregation](https://developer.apple.com/videos/play/wwdc2026/222/) | 27 (DocC verified on macOS) | Field metrics and diagnostics after deployment. Aggregated app metrics cannot replace GPU counters. |
| P04 | [gpucapture; gpudebug](https://developer.apple.com/videos/play/wwdc2026/357/); [metalperftrace collect / overview](https://developer.apple.com/videos/play/wwdc2026/388/) | 27 | CLI capture and shader inspection. `metalperftrace` records only CAMetalLayer frame statistics and shows nothing for headless compute. |

### Core AI

| ID | API / capability | Era | Notes and limits |
| --- | --- | --- | --- |
| A01 | [AIModel; InferenceFunction; NDArray; loadFunction / run](https://developer.apple.com/videos/play/wwdc2026/324/) | 27 | Independent compiler/runtime baseline. Requires conversion and supported graph semantics. |
| A02 | [InferenceFunction.MutableViews; run(inputs:states:outputViews:)](<https://developer.apple.com/documentation/CoreAI/InferenceFunction/run(inputs:states:outputViews:)-14emi>) | 27 | Reuses state and output allocations for repeated inference. State ownership, aliasing and output omission rules matter. |
| A03 | [AsyncValue; AsyncMutableValue; AsyncMutableViews; encode(inputs:states:outputViews:to:)](https://developer.apple.com/documentation/CoreAI/InferenceFunction/AsyncMutableValue) | 27 | Queues a sequence without unnecessary host waits. Core AI uses its own async inference API; do not assume its encoder is an MTL4CommandBuffer. |
| A04 | NDArray views over shared MTLBuffer, spans and IOSurface; preferred layout queries (SDK: `NDArray.MutableRawView(metalBuffer:)`, `(ioSurface:)`, `NDArrayDescriptor.preferredStrides`) | 27 | Interop with existing allocations. Whether foreign-memory views trigger internal copies is unverified. |
| A05 | [AIModel.specialize; SpecializationOptions; AIModelCache; coreai-build; deviceArchitectureName](https://developer.apple.com/documentation/CoreAI/managing-model-specialization-and-caching) | 27 | Separate cold compilation, warm loading and steady inference. AOT still leaves device specialization; the cache is OS/device dependent. |
| A06 | [coreai_torch.TorchConverter; add_exported_program](https://developer.apple.com/videos/play/wwdc2026/325/); [get_decomp_table; state_names](https://developer.apple.com/videos/play/wwdc2026/324/) | 27 | Convert only the needed subgraph first. Avoid decomposing away recognized attention/state semantics. |
| A07 | [TorchMetalKernel; MetalParameter; register_custom_kernels](https://developer.apple.com/videos/play/wwdc2026/325/) | 27 | Embeds custom Metal kernels in a converted model. Custom kernels may form fusion boundaries; measure the complete path. |
| A08 | [coreai-opt Quantizer; ExecutionMode EAGER / GRAPH; KMeansPalettizer; presets.w4](https://developer.apple.com/videos/play/wwdc2026/325/) | 27 | Compression changes the weight or activation representation and needs a quality evaluation. |
| A09 | [Core AI Debugger; saved intermediates / sync-point comparisons](https://developer.apple.com/documentation/CoreAI/validating-inference-correctness-against-a-reference-run) | 27 | Locates numerical divergence at graph boundaries. Not a debugger for arbitrary AIR; requires Core AI artifacts. |

### CPU graphs

| ID | API / capability | Era | Notes and limits |
| --- | --- | --- | --- |
| B01 | [BNNSGraph.makeContext / Builder; typed tensor expressions; SliceRange](https://developer.apple.com/videos/play/wwdc2025/276/) | 26 | CPU API for small latency-sensitive stages; no access to the GPU neural accelerators. |

### MLX

| ID | API / capability | Era | Notes and limits |
| --- | --- | --- | --- |
| X01 | [mx.compile; mx.fast.metal_kernel; quantized_matmul; quant_predicate; prompt cache](https://developer.apple.com/videos/play/wwdc2025/315/) [also 1](https://developer.apple.com/videos/play/wwdc2025/298/) | WWDC25 showcased; do not assume every symbol debuted in 26 | External reference implementation. Match quantization and timing boundaries before comparing. |
| X02 | [JACCL; distributed.init(backend=jaccl); all_sum; sharded_load / shard_linear](https://developer.apple.com/videos/play/wwdc2026/233/) | RDMA in 26.2; WWDC26 showcase | Multi-Mac only; needs multiple machines and explicit network setup. Not a single-machine optimization. |

### Adjacent ML

| ID | API / capability | Era | Notes and limits |
| --- | --- | --- | --- |
| F01 | [LanguageModelSession; SystemLanguageModel; @Generable / @Guide; Tool; DynamicGenerationSchema](https://developer.apple.com/videos/play/wwdc2025/286/) [also 1](https://developer.apple.com/videos/play/wwdc2025/301/) | 26 | App-level framework with its own serving contract; not a kernel API. |
| F02 | [LanguageModel / LanguageModelExecutor; capabilities; generation request/channel](https://developer.apple.com/videos/play/wwdc2026/339/) | 27 | Lets an app plug in its own LLM provider. Integration API; no GPU compute content. |
| F03 | [LanguageModelSession.DynamicProfile; DynamicInstructions; SessionProperty; historyTransform](https://developer.apple.com/videos/play/wwdc2026/242/) | 27 | App-level routing/history management. Changing history changes inference work. |
| F04 | [contextSize; tokenCount(for:); response.usage; ContextOptions; Attachment](https://developer.apple.com/videos/play/wwdc2026/241/) | 27 | App-side context/usage instrumentation. Framework token counts are not engine counts. |
| F05 | [Evaluation; ModelSample; Evaluator; Metric; ModelJudgeEvaluator; SampleGenerator; ToolCallEvaluator](https://developer.apple.com/videos/play/wwdc2026/298/) [also 1](https://developer.apple.com/videos/play/wwdc2026/299/) | 27 | Model quality evaluation. Does not replace bitwise kernel checks or calibrated latency tests. |
| F06 | [SpeechAnalyzer; SpeechTranscriber](https://developer.apple.com/videos/play/wwdc2025/277/) | 26 | Different workload; listed for completeness. |
| F07 | [RecognizeDocumentsRequest; GenerateIterativeSegmentationRequest](https://developer.apple.com/videos/play/wwdc2026/237/) [also 1](https://developer.apple.com/videos/play/wwdc2025/272/) | 26 / 27 | Different workload; listed for completeness. |
| F08 | [MusicUnderstandingSession](https://developer.apple.com/videos/play/wwdc2026/253/) | 27 | Different workload; listed for completeness. |
| F09 | [SpotlightSearchTool; apple_fm_sdk / fm CLI](https://developer.apple.com/videos/play/wwdc2026/246/) [also 1](https://developer.apple.com/videos/play/wwdc2026/334/) | 27 | Search/provider tooling; no GPU compute content. |

### Rendering only

| ID | API / capability | Era | Notes and limits |
| --- | --- | --- | --- |
| G01 | [MTL4RenderPassDescriptor; color attachment mapping; suspend/resume; flexible render pipelines; MTLTextureViewPool](https://developer.apple.com/videos/play/wwdc2025/254/) | 26 | Rendering functionality, not compute/ML. |
| G02 | [MTLFXFrameInterpolatorDescriptor; denoised temporal upscaling; ray-tracing additions](https://developer.apple.com/videos/play/wwdc2025/211/) | 26 / 27 extensions | Rendering functionality, not compute/ML. |

## Session catalog

The lists below include adjacent sessions so omissions are visible. A title in this catalog is not a claim of a new usable kernel API. App Intents, Siri UI, rendering-only features, networked providers and group-lab discussion are retained for context rather than presented as GPU compute APIs.

### WWDC25

| Session | Scope in this audit | Platforms in catalog |
| --- | --- | --- |
| [Bring advanced speech-to-text to your app with SpeechAnalyzer](https://developer.apple.com/videos/play/wwdc2025/277/) | Adjacent ML API | ios, ipados, macos |
| [Code-along: Bring on-device AI to your app using the Foundation Models framework](https://developer.apple.com/videos/play/wwdc2025/259/) | Adjacent ML API | ios, ipados, macos |
| [Combine Metal 4 machine learning and graphics](https://developer.apple.com/videos/play/wwdc2025/262/) | Engine/runtime/tools | ios, ipados, macos |
| [Deep dive into the Foundation Models framework](https://developer.apple.com/videos/play/wwdc2025/301/) | Adjacent ML API | ios, ipados, macos |
| [Design interactive snippets](https://developer.apple.com/videos/play/wwdc2025/281/) | Application/UI context | ios, ipados, macos, visionos, watchos |
| [Develop for Shortcuts and Spotlight with App Intents](https://developer.apple.com/videos/play/wwdc2025/260/) | Application/UI context | ios, ipados, macos |
| [Discover machine learning & AI frameworks on Apple platforms](https://developer.apple.com/videos/play/wwdc2025/360/) | Engine/runtime/tools | ios, ipados, macos |
| [Discover Metal 4](https://developer.apple.com/videos/play/wwdc2025/205/) | Engine/runtime/tools | ios, ipados, macos, tvos, visionos |
| [Dive deeper into Writing Tools](https://developer.apple.com/videos/play/wwdc2025/265/) | Application/UI context | ios, ipados, macos, visionos |
| [Explore large language models on Apple silicon with MLX](https://developer.apple.com/videos/play/wwdc2025/298/) | Engine/runtime/tools | ios, ipados, macos |
| [Explore Metal 4 games](https://developer.apple.com/videos/play/wwdc2025/254/) | Engine/runtime/tools | ios, ipados, macos, tvos, visionos |
| [Explore new advances in App Intents](https://developer.apple.com/videos/play/wwdc2025/275/) | Application/UI context | ios, ipados, macos |
| [Explore prompt design & safety for on-device foundation models](https://developer.apple.com/videos/play/wwdc2025/248/) | Application/UI context | ios, ipados, macos |
| [Get started with MLX for Apple silicon](https://developer.apple.com/videos/play/wwdc2025/315/) | Engine/runtime/tools | ios, ipados, macos |
| [Get to know App Intents](https://developer.apple.com/videos/play/wwdc2025/244/) | Application/UI context | ios, ipados, macos |
| [Go further with Metal 4 games](https://developer.apple.com/videos/play/wwdc2025/211/) | Engine/runtime/tools | ios, ipados, macos, tvos, visionos |
| [Meet the Foundation Models framework](https://developer.apple.com/videos/play/wwdc2025/286/) | Adjacent ML API | ios, ipados, macos |
| [Optimize CPU performance with Instruments](https://developer.apple.com/videos/play/wwdc2025/308/) | Engine/runtime/tools | ios, ipados, macos, tvos, visionos, watchos |
| [Profile and optimize power usage in your app](https://developer.apple.com/videos/play/wwdc2025/226/) | Engine/runtime/tools | ios, ipados |
| [Read documents using the Vision framework](https://developer.apple.com/videos/play/wwdc2025/272/) | Adjacent ML API | ios, ipados, macos, visionos |
| [Unlock GPU computing with WebGPU](https://developer.apple.com/videos/play/wwdc2025/236/) | Engine/runtime/tools | ios, ipados, macos, visionos |
| [What’s new in BNNS Graph](https://developer.apple.com/videos/play/wwdc2025/276/) | Engine/runtime/tools | ios, ipados, macos |
| [What’s new in Metal rendering for immersive apps](https://developer.apple.com/videos/play/wwdc2025/294/) | Engine/runtime/tools | macos, visionos |
| [What’s new in Xcode 26](https://developer.apple.com/videos/play/wwdc2025/247/) | Engine/runtime/tools | ios, ipados, macos, tvos, visionos, watchos |

### WWDC26

| Session | Scope in this audit | Platforms in catalog |
| --- | --- | --- |
| [Apple Intelligence Group Lab](https://developer.apple.com/videos/play/wwdc2026/8011/) | Q&A / overview | not tagged |
| [Best practices for integrating visual intelligence in your app](https://developer.apple.com/videos/play/wwdc2026/297/) | Application/UI context | ios, ipados, macos |
| [Bring an LLM provider to the Foundation Models framework](https://developer.apple.com/videos/play/wwdc2026/339/) | Adjacent ML API | ios, ipados, macos, visionos |
| [Bringing Cyberpunk 2077 to Mac](https://developer.apple.com/videos/play/wwdc2026/356/) | Engine/runtime/tools | macos |
| [Build agentic app experiences with the Foundation Models framework](https://developer.apple.com/videos/play/wwdc2026/242/) | Adjacent ML API | ios, ipados, macos, visionos |
| [Build AI-powered scripts with the fm CLI and Python SDK](https://developer.apple.com/videos/play/wwdc2026/334/) | Adjacent ML API | macos |
| [Build intelligent Siri experiences with App Schemas](https://developer.apple.com/videos/play/wwdc2026/240/) | Application/UI context | ios, ipados, macos, visionos |
| [Build real-time neural rendering pipelines with Metal](https://developer.apple.com/videos/play/wwdc2026/359/) | Engine/runtime/tools | ios, ipados, macos |
| [Build with the new Apple Foundation Model on Private Cloud Compute](https://developer.apple.com/videos/play/wwdc2026/319/) | Adjacent ML API | ios, ipados, macos, visionos |
| [Code-along: Make your app available to Siri](https://developer.apple.com/videos/play/wwdc2026/344/) | Application/UI context | ios, ipados, macos |
| [Coding Intelligence, Machine Learning & AI Group Lab](https://developer.apple.com/videos/play/wwdc2026/8121/) | Q&A / overview | not tagged |
| [Create high-quality images using Image Playground](https://developer.apple.com/videos/play/wwdc2026/375/) | Application/UI context | ios, ipados, macos, visionos |
| [Create robust evaluations for agentic apps](https://developer.apple.com/videos/play/wwdc2026/299/) | Application/UI context | ios, ipados, macos, visionos |
| [Create UI prototypes using agents in Xcode](https://developer.apple.com/videos/play/wwdc2026/227/) | Application/UI context | ios, ipados, macos |
| [Debug and profile agentic app experiences with Instruments](https://developer.apple.com/videos/play/wwdc2026/243/) | Engine/runtime/tools | ios, ipados, macos |
| [Design immersive environments for visionOS apps and the spatial web](https://developer.apple.com/videos/play/wwdc2026/234/) | Application/UI context | visionos |
| [Discover new capabilities in the App Intents framework](https://developer.apple.com/videos/play/wwdc2026/345/) | Application/UI context | ios, ipados, visionos |
| [Dive into Core AI model authoring and optimization](https://developer.apple.com/videos/play/wwdc2026/325/) | Engine/runtime/tools | ios, ipados, macos, watchos |
| [Explore advanced App Intents features for Siri and Apple Intelligence](https://developer.apple.com/videos/play/wwdc2026/343/) | Application/UI context | ios, ipados, visionos |
| [Explore distributed inference and training with MLX](https://developer.apple.com/videos/play/wwdc2026/233/) | Engine/runtime/tools | macos |
| [Explore numerical computing in Swift with MLX](https://developer.apple.com/videos/play/wwdc2026/328/) | Engine/runtime/tools | macos |
| [Find and fix performance issues in your Metal games](https://developer.apple.com/videos/play/wwdc2026/388/) | Engine/runtime/tools | ios, ipados, macos |
| [Improve your prompts by hill-climbing with Evaluations](https://developer.apple.com/videos/play/wwdc2026/335/) | Adjacent ML API | ios, ipados, macos, visionos |
| [Inside Apple Intelligence and Xcode: Special Presentation](https://developer.apple.com/videos/play/wwdc2026/382/) | Q&A / overview | ios, ipados, macos, tvos, visionos, watchos |
| [Integrate on-device AI models into your app using Core AI](https://developer.apple.com/videos/play/wwdc2026/326/) | Engine/runtime/tools | ios, ipados, macos, watchos |
| [LLM search using Core Spotlight](https://developer.apple.com/videos/play/wwdc2026/246/) | Adjacent ML API | ios, ipados, macos |
| [Machine Learning & AI Group Lab](https://developer.apple.com/videos/play/wwdc2026/8016/) | Q&A / overview | not tagged |
| [Meet Core AI](https://developer.apple.com/videos/play/wwdc2026/324/) | Engine/runtime/tools | ios, ipados, macos, watchos |
| [Meet the Evaluations framework](https://developer.apple.com/videos/play/wwdc2026/298/) | Adjacent ML API | ios, ipados, macos, visionos |
| [Meet the Music Understanding framework](https://developer.apple.com/videos/play/wwdc2026/253/) | Adjacent ML API | ios, ipados, macos, visionos, watchos |
| [Meet the new MetricKit](https://developer.apple.com/videos/play/wwdc2026/222/) | Engine/runtime/tools | ios, ipados, macos |
| [Optimize custom machine learning operations with Metal tensors](https://developer.apple.com/videos/play/wwdc2026/330/) | Engine/runtime/tools | ios, ipados, macos, tvos, visionos, watchos |
| [Profile, fix, and verify: Improve app responsiveness with Instruments](https://developer.apple.com/videos/play/wwdc2026/268/) | Engine/runtime/tools | ios, ipados, macos, tvos, visionos, watchos |
| [Run local agentic AI on the Mac using MLX](https://developer.apple.com/videos/play/wwdc2026/232/) | Engine/runtime/tools | macos |
| [Secure your app: mitigate risks to agentic features](https://developer.apple.com/videos/play/wwdc2026/347/) | Application/UI context | ios, ipados, macos, tvos, visionos, watchos |
| [Speedrun your game port with agentic coding](https://developer.apple.com/videos/play/wwdc2026/357/) | Engine/runtime/tools | ios, ipados, macos |
| [Translate your app using agents in Xcode](https://developer.apple.com/videos/play/wwdc2026/213/) | Application/UI context | ios, ipados, macos, tvos, visionos, watchos |
| [Validate your App Intents adoption with AppIntentsTesting](https://developer.apple.com/videos/play/wwdc2026/295/) | Application/UI context | ios, ipados |
| [What’s new in image understanding](https://developer.apple.com/videos/play/wwdc2026/237/) | Adjacent ML API | ios, ipados, macos, visionos, watchos |
| [What’s new in Shortcuts](https://developer.apple.com/videos/play/wwdc2026/310/) | Application/UI context | ios, ipados, macos, visionos, watchos |
| [What’s new in the Foundation Models framework](https://developer.apple.com/videos/play/wwdc2026/241/) | Adjacent ML API | ios, ipados, macos, tvos, visionos |
| [What’s new in Xcode 27](https://developer.apple.com/videos/play/wwdc2026/258/) | Engine/runtime/tools | ios, ipados, macos, tvos, visionos, watchos |

### Companion materials

- [Accelerate your machine learning workloads with the M5 and A19 GPUs](https://developer.apple.com/videos/play/tech-talks/111432/) — TensorOps tiling (explicit K tiling, periodic barriers, Morton/Hilbert order) and the neural-accelerator utilization counter in Metal System Trace; a technical talk (2026-03-20), not a WWDC session.
- [Boost your graphics performance with the M5 and A19 GPUs](https://developer.apple.com/videos/play/tech-talks/111431/) — architecture and profiling context; graphics features are not automatically useful for inference.
- [Metal Performance Primitives programming guide](https://developer.apple.com/download/files/Metal-Performance-Primitives-Programming-Guide.pdf) and [feature tables](https://developer.apple.com/metal/capabilities/) — verify operation, type, family and alignment contracts.
- [Metal 4 core API](https://developer.apple.com/documentation/metal/understanding-the-metal-4-core-api), [inline ML sample](https://developer.apple.com/documentation/metal/running-inline-ml-operations-in-a-shader-with-metal-4), and [machine-learning passes](https://developer.apple.com/documentation/metal/machine-learning-passes).
- [Core AI model inspection](https://developer.apple.com/documentation/coreai/inspecting-debugging-and-profiling-core-ai-models) — applies to converted model graphs, not arbitrary metallibs.

## Audit artifacts and limits

[wwdc-api-audit.json](evidence/2026-10-11/wwdc-api-audit.json) keeps the catalog counts, scope counts and file hashes; the raw catalog, transcripts and availability checks were not kept. The WWDC25 and WWDC26 video catalogs are the discovery boundary; older related sessions are context only. No ML-encoder pipeline or Core AI conversion was run for this chapter.
