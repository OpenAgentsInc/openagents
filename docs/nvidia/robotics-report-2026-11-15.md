# NVIDIA’s robotics efforts: full report

**Research cutoff: November 15, 2026**

NVIDIA is building the computing platform used to develop, train, simulate, deploy, and improve robots. Its efforts span far more than humanoids: industrial arms, warehouse fleets, medical systems, drones, heavy equipment, autonomous vehicles, and the infrastructure behind them. The portfolio includes onboard computers, physics engines, simulation software, robot foundation models, synthetic data tools, perception and motion libraries, fleet systems, and safety technology. ([NVIDIA Robotics](https://www.nvidia.com/en-us/industries/robotics/), [Isaac](https://developer.nvidia.com/isaac))

The report’s central assessment is that NVIDIA’s greatest advantage comes from connecting the stages of robotics development. A company can use NVIDIA to train its own models, simulate its robots, accelerate perception, and run onboard inference without adopting NVIDIA’s entire stack. That gives NVIDIA several ways to participate in the industry’s growth.

This is a dated research snapshot. Released products, research previews, reference designs, pilots, and planned releases are distinguished where relevant. Availability, specifications, licensing, and forward-looking statements should be rechecked before implementation.

## 1. The strategy: provide the infrastructure across the robot’s lifecycle

NVIDIA organizes its robotics architecture around three computational roles:

1. **Training:** develop perception models, robot policies, and other learned components.
2. **Simulation:** generate experience, test policies, reproduce environments, and evaluate failures.
3. **Deployment:** run sensing, inference, planning, and control on the physical machine.

Its “three-computer” terminology describes these responsibilities. A small team can combine some roles on one workstation; a large manufacturer might distribute them across datacenters, simulation clusters, and thousands of onboard computers. ([NVIDIA Robotics](https://www.nvidia.com/en-us/industries/robotics/), [Isaac Sim reference architecture](https://docs.isaacsim.omniverse.nvidia.com/6.2.0/introduction/reference_architecture.html))

The commercially significant part is the feedback loop. A robot encounters a failure; engineers reconstruct it, add it to evaluation, improve the data or policy, and deploy an update. NVIDIA supplies components throughout that process.

### A map of the portfolio

| Layer | Principal NVIDIA efforts | Purpose |
| --- | --- | --- |
| Robot intelligence | GR00T; specialized healthcare and driving models | Convert observations and instructions into actions |
| Physical-world models | Cosmos | Understand scenes, generate possible outcomes, and support training |
| Data collection | Isaac Capture, Teleop, Human-1M | Collect demonstrations and human activity data |
| Synthetic data | Replicator, MimicGen, DreamGen, MobilityGen | Expand training data and environmental variation |
| Robot simulation | Isaac Sim, Omniverse, NuRec | Reproduce robots, environments, and sensors |
| Physics and learning | PhysX, Newton, Warp, Isaac Lab | Simulate dynamics and train policies |
| Perception and motion | cuVSLAM, nvblox, cuMotion, cuRobo | Localize, reconstruct surroundings, and plan movement |
| Robot runtime | Isaac ROS, Isaac OS, TensorRT, Holoscan | Execute the robot’s software |
| Onboard hardware | Jetson Orin, Jetson Thor, IGX | Supply embedded AI and sensor-processing compute |
| Development orchestration | OSMO, Physical AI Data Factory | Coordinate datasets, training, simulation, and evaluation |
| Fleets and facilities | Mission Dispatch/Control, cuOpt, Mega | Coordinate missions and model industrial operations |
| Safety | Halos for Robotics, IGX safety capabilities | Support safety functions and system validation |

These are composable layers. “Isaac,” “Omniverse,” and “Cosmos” are umbrella names, which explains some of the apparent overlap in NVIDIA’s product announcements. ([Isaac](https://developer.nvidia.com/isaac), [Omniverse](https://developer.nvidia.com/omniverse))

My assessment is that the resulting advantage has three parts: accelerated computing, integration among development tools, and the accumulated experience of developers already using those tools. The latter two can create substantial switching costs even where source code is available.

## 2. Simulation: Isaac Sim, Isaac Lab, PhysX, Newton, and Warp

Simulation is central because physical robot experiments are comparatively slow, costly, and difficult to reproduce.

### Isaac Sim

**Isaac Sim is the robot development and simulation environment.** It brings together robot and scene assets, physics, simulated cameras and other sensors, rendering, visualization, synthetic data generation, and connections to robot software. It supports closed-loop testing and workflows in which a physical onboard computer receives simulated sensor inputs. ([Reference architecture](https://docs.isaacsim.omniverse.nvidia.com/6.2.0/introduction/reference_architecture.html))

Isaac Sim **6.2 became generally available on October 22, 2026**. Its release emphasized reliability and support for Isaac Lab 3.0, including fixes affecting Newton, robot assets, and teleoperation. ([Availability announcement](https://forums.developer.nvidia.com/t/isaac-sim-6-2-general-availability/386512))

For users, the distinction between a convincing visual scene and a useful robot simulator matters. Navigation may depend on camera timing, depth errors, and reflective surfaces. Assembly may depend on friction, compliance, tolerances, and contact forces. A beautiful scene can still be unsuitable for validating either task.

### Isaac Lab

**Isaac Lab is the learning framework.** It defines observations, actions, rewards, resets, robot configurations, and interfaces to learning algorithms. It supports reinforcement learning, imitation learning, and related policy-development workflows. Version 3.0 introduced a more modular architecture for physics and rendering backends. ([Isaac Lab repository](https://github.com/isaac-sim/IsaacLab))

A significant architectural change is that Lab can use Newton **without requiring the full Isaac Sim application**. This helps separate lightweight physics training from workloads needing elaborate sensor rendering. Isaac Gym and its associated older workflows are now legacy starting points. ([Isaac platform overview](https://docs.nvidia.com/learning/physical-ai/getting-started-with-isaac-lab/2026-10.0/isaac-lab-basics/01-the-isaac-platform.html), [Isaac Gym](https://developer.nvidia.com/isaac-gym/))

### The physics and computation layers

| Technology | Role | Why it matters |
| --- | --- | --- |
| **PhysX** | Established physics SDK | Rigid-body dynamics and other simulation capabilities; current interfaces include `ovphysx` |
| **Newton** | Extensible robotics physics engine | GPU-oriented simulation, multiple solver capabilities, and research extensibility |
| **Warp** | Computational kernel framework | Custom geometry, physics, numerical computation, and differentiable programming |
| **Isaac Lab** | Learning environment framework | Connects robot tasks and simulation backends to policy training |

PhysX’s current repository exposes an `ovphysx` C API, Python bindings, and tensor interoperability. That creates an integration option for developers building their own applications. ([PhysX repository](https://github.com/NVIDIA-Omniverse/PhysX))

Newton is an open-source project developed with **Google DeepMind and Disney Research**, hosted under the Linux Foundation. It builds on Warp and integrates capabilities including MuJoCo Warp. Warp compiles Python-authored computational kernels for CPU or NVIDIA GPU execution. ([Newton repository](https://github.com/newton-physics/newton), [Warp repository](https://github.com/NVIDIA/warp))

### Omniverse, OpenUSD, and NuRec

**OpenUSD** supplies scene description and composition: relationships among geometry, materials, robot parts, and properties. **Omniverse** supplies surrounding rendering, scene, storage, and simulation capabilities, increasingly through individual libraries such as `ovrtx` and `ovstage`. ([Omniverse](https://developer.nvidia.com/omniverse))

**NuRec** reconstructs environments from real observations using neural representations, including 3D Gaussian splatting. This can accelerate creation of test environments. A reconstructed appearance still needs suitable collision geometry, articulation, and physical properties before it becomes a dependable interaction environment. ([NuRec](https://developer.nvidia.com/omniverse/nurec))

The practical implication is that teams can adopt selected pieces: exchange USD assets, call a physics library, or run a separate simulator service. Adopting an entire application framework is a larger architectural choice.

## 3. Robot foundation models: GR00T

GR00T is NVIDIA’s effort to create adaptable robot policies that learn across tasks and embodiments.

A vision-language-action model, or **VLA**, receives visual observations, language instructions, and robot state, then predicts actions. It can reduce the amount of task-specific programming, but still requires compatible action definitions, calibrated sensors, and a controller capable of executing its outputs.

### GR00T N1.7: the released model

As of the report date, **GR00T N1.7 is the released 3-billion-parameter model**. It combines a vision-language backbone with a flow-matching action transformer, using vision, language, and proprioception. Model weights permit commercial and noncommercial use under the NVIDIA Open Model License. ([GR00T N1.7 model card](https://huggingface.co/nvidia/GR00T-N1.7-3B))

The current implementation uses a Cosmos-Reason2-2B backbone with the Qwen3-VL architecture and provides ONNX/TensorRT deployment paths. Repository guidance lists at least 16GB of GPU memory for inference, with more recommended for fine-tuning. ([Isaac GR00T repository](https://github.com/NVIDIA/Isaac-GR00T))

A deployment typically involves collecting demonstrations, translating them into the expected dataset and action formats, adapting the model, evaluating it, and integrating it with the physical robot. NVIDIA’s workflow includes LeRobot-format data and whole-body control where required. ([End-to-end policy development](https://developer.nvidia.com/blog/develop-humanoid-robot-policies-end-to-end-with-nvidia-isaac-gr00t/))

### GR00T 2: the next research generation

**GR00T 2 was introduced in November 2026, with code scheduled for December.** It should therefore be treated as a research preview at this cutoff.

Its distinguishing approach is to predict future video and robot actions together. This “world-action model” learns expected interactions alongside the actions that produce them. NVIDIA reports mean success of **40% without task-specific demonstrations on the target robot**, increasing to **95% after five demonstrations per task**, across its 50-task evaluation. ([GR00T 2 research](https://research.nvidia.com/labs/gear/gr00t-2/))

Those are author-reported experimental results. The separate statement that the model succeeded at least once on 40 of 50 tasks describes task coverage; it should not be interpreted as an 80% trial success rate. The CoRL announcements also broadened NVIDIA’s evaluation and industrial dexterity work. ([CoRL robotics announcements](https://blogs.nvidia.com/blog/robotics-simulation-advancements-corl/))

The research direction is consequential: human video is much more plentiful than robot action data. Learning from it could reduce demonstration requirements. Differences in reach, grip, force, tactile sensing, balance, and controller behavior remain important.

### A robot still has multiple control layers

A useful system can combine a task planner, an action-generating policy, a whole-body controller, and fast actuator loops. NVIDIA’s **SONIC** research addresses generalist whole-body motion tracking and the connection between higher-level targets and coordinated humanoid movement. ([SONIC](https://nvlabs.github.io/GEAR-SONIC/))

Consequently, model inference frequency and motor-control frequency are different measurements. A fast policy benchmark alone says little about end-to-end response time or recovery from failure.

## 4. Cosmos: models of the physical world

Cosmos supplies models and tools for physical-world understanding and generation. Existing workflows use the names **Predict**, **Transfer**, and **Reason** for future prediction, controlled visual variation, and scene reasoning. The current **Cosmos 3** family combines reasoning and multimodal generation through a Mixture of Transformers architecture. ([Cosmos 3 research](https://research.nvidia.com/labs/cosmos-lab/cosmos3/))

| Cosmos 3 model | Parameter count | Principal positioning |
| --- | --- | --- |
| **Super** | 64 billion | High-quality generation and teacher-model workflows |
| **Nano** | 16 billion | A balance of capability and compute requirements |
| **Edge** | 4 billion | On-device visual reasoning and robot-policy workflows |

All three have available artifacts in the current model matrix. Their deployment requirements and capabilities differ. ([Cosmos 3 model matrix](https://docs.nvidia.com/cosmos/latest/cosmos3/model_matrix.html))

The surrounding platform includes **Cosmos Framework** for training and adaptation, **Curator** for preparing data, and **Evaluator** for assessing outputs. Current Cosmos 3 source and models use OpenMDW-1.1. ([Cosmos repository](https://github.com/NVIDIA/cosmos))

Cosmos can help generate varied training scenarios, assess observations, and supply learned representations for embodied systems. Its model card explicitly acknowledges limits in geometry, contact, physical consistency, and reasoning. ([Cosmos 3 Edge model card](https://huggingface.co/nvidia/Cosmos3-Edge))

My assessment is that its strongest use is alongside geometric simulation and real-world measurements. Generated diversity is valuable; physical feasibility and control performance still need separate evidence.

## 5. Data collection and synthetic data

NVIDIA is investing heavily in the data problem because robot demonstrations are tied to particular bodies, sensors, controllers, and environments.

### The main pipelines

| Effort | What it contributes |
| --- | --- |
| **Isaac Capture / Teleop** | Device input, teleoperation, retargeting, and human egocentric capture |
| **MimicGen / GR00T-Mimic** | Additional task trajectories derived from a limited demonstration set |
| **DexMimicGen** | Demonstration amplification for coordinated, dexterous two-arm manipulation |
| **Replicator** | Rendered sensor data, annotations, and randomized scene conditions |
| **MobilityGen** | Mobile-robot trajectory recording, replay, and ground-truth sensor data |
| **DreamGen / GR00T-Dreams** | Generated behavior videos and inferred actions for policy training |
| **Human-1M** | Human activity video for embodied learning |
| **SimReady Central** | Discoverable simulation asset packages |

Isaac Capture now groups teleoperation and egocentric capture infrastructure. MimicGen expands demonstrations through structured task variations; DexMimicGen extends that approach to more complex bimanual work. ([Isaac Capture](https://github.com/NVIDIA/IsaacCapture), [MimicGen](https://mimicgen.github.io/), [DexMimicGen](https://dexmimicgen.github.io/))

Replicator and MobilityGen address sensor-data generation, while DreamGen studies adapting video models, generating behavior, inferring pseudo-actions, and training downstream policies. ([Replicator tutorials](https://docs.isaacsim.omniverse.nvidia.com/latest/replicator_tutorials/index.html), [MobilityGen](https://docs.isaacsim.omniverse.nvidia.com/latest/synthetic_data_generation/tutorial_replicator_mobility_gen.html), [DreamGen](https://research.nvidia.com/labs/gear/dreamgen/))

Two availability distinctions are important:

- **Human-1M-100K** is the public 100,000-hour subset, under CC BY 4.0. The full Human-1M dataset has a partner-access arrangement.
- **SimReady Central** is labeled alpha, and contributed assets retain their individual licenses. ([Human-1M-100K](https://huggingface.co/datasets/nvidia/PhysicalAI-Human-1M-100K), [SimReady Central](https://huggingface.co/datasets/nvidia/simready-central-library))

For engineering teams, provenance is crucial. Real demonstrations, generated trajectories, rendered images, and model-labeled examples should remain distinguishable. Otherwise, closely related generated material can leak into both training and evaluation and make generalization appear stronger than it is.

## 6. Research beyond the flagship products

NVIDIA’s robotics research is broader than GR00T. Its GEAR group connects multimodal models, robot learning, simulation, and agents that learn through interaction. Other projects address contact mechanics, assembly, perception, dexterity, and control. ([GEAR research group](https://research.nvidia.com/labs/gear/))

| Research program | Main contribution |
| --- | --- |
| **Eureka** | Language-model generation and improvement of reinforcement-learning reward code |
| **DrEureka** | Language-model assistance for domain randomization and transfer to real robots |
| **DreamZero** | World-action models that jointly predict video and actions |
| **DreamDojo** | Action-conditioned world simulation learned from human video |
| **SONIC** | Whole-body humanoid motion tracking |
| **Factory, IndustReal, AutoMate** | Contact-rich simulation, assembly learning, and transfer among geometries |
| **TacSL and FORGE** | Tactile simulation and force-sensitive manipulation |
| **GRAIL** | Grounding generated interactions in humanoid motion |
| **RoboTTT** | Longer-context policies using recurrent fast-weight updates |

Eureka and DrEureka investigate using language models to help design the learning process itself. DreamZero and DreamDojo investigate predictive models of interaction. ([Eureka](https://eureka-research.github.io/), [DrEureka](https://eureka-research.github.io/dr-eureka/), [DreamZero](https://dreamzero0.github.io/), [DreamDojo](https://dreamdojo-world.github.io/))

The assembly research tackles the physical detail that broad foundation models still need: precise contact, insertion, force, and tactile behavior. GRAIL and RoboTTT address additional challenges in motion grounding and policy memory. ([Contact-rich assembly research](https://developer.nvidia.com/blog/r2d2-unlocking-robotic-assembly-and-contact-rich-manipulation-with-nvidia-research/), [GRAIL](https://research.nvidia.com/labs/dair/grail/), [RoboTTT](https://research.nvidia.com/labs/gear/robottt/))

These projects have differing maturity and support levels. A published research implementation should be evaluated on its own terms before becoming a production dependency.

## 7. Hardware: Jetson, Thor, IGX, and the development computers

### Jetson

Jetson combines an Arm CPU, NVIDIA GPU, memory, embedded interfaces, and the JetPack software environment.

| Product | Headline specification | Status at report cutoff |
| --- | --- | --- |
| **Orin Nano Super** | Up to 67 TOPS | Available |
| **Orin NX 16GB** | Up to 157 TOPS | Available |
| **AGX Orin** | Up to 275 TOPS; up to 64GB memory | Available |
| **Thor T5000** | 2,070 sparse FP4 TFLOPS; 128GB | Available |
| **Thor T4000** | 1,200 sparse FP4 TFLOPS; 64GB | Available |
| **Thor T3000** | 865 FP4 TFLOPS; 32GB | Announced; Q1 2027 expected |
| **Thor T2000** | 400 FP4 TFLOPS; 16GB | Announced; Q1 2027 expected |
| **Orin Nano 2** | 78 TOPS; 8GB | Announced; first half of 2027 expected |

The available Orin and Thor products span a wide range of onboard workloads; the lower-cost announced additions extend that range but should not be counted as shipping products yet. ([Jetson Thor](https://www.nvidia.com/en-us/autonomous-machines/embedded-systems/jetson-thor/), [Jetson Orin](https://www.nvidia.com/en-us/autonomous-machines/embedded-systems/jetson-orin/), [Thor robotics and edge AI](https://blogs.nvidia.com/blog/jetson-thor-robotics-edge-ai-agent/), [Orin Nano 2 announcement](https://nvidianews.nvidia.com/news/nvidia-announces-jetson-orin-nano-2-robotics-computer-to-redefine-entry-level-edge-ai))

**TOPS and sparse FP4 TFLOPS are not directly comparable measures of robot performance.** Precision, sparsity, memory traffic, software, and actual model utilization differ. Thor T5000’s advertised configuration includes 128GB memory, 273GB/s bandwidth, and a configurable 40–130W power range. ([Jetson Thor specifications](https://www.nvidia.com/en-us/autonomous-machines/embedded-systems/jetson-thor/))

The current Jetson FAQ lists developer-kit prices of **$5,499 for Thor, $3,499 for AGX Orin, and $399 for Orin Nano Super**. These are development products and prices, not complete robot costs or production-module quotations. ([Jetson FAQ](https://developer.nvidia.com/embedded/faq))

### JetPack and development infrastructure

JetPack supplies the Jetson Linux environment and accelerated software stack. Current downloads list **JetPack 7.2.1**, including CUDA 13.2.2 and TensorRT 10.16.2. Teams need to select a tested combination of hardware, operating system, drivers, runtime, and robotics packages. ([JetPack downloads](https://developer.nvidia.com/embedded/jetpack/downloads))

Training and simulation use NVIDIA’s broader datacenter and workstation products. For many teams, access to suitable GPUs, sufficient memory, and reproducible environments matters more than buying the largest system.

### IGX and Holoscan

**IGX** targets industrial and medical deployments with enterprise support and safety-related capabilities. IGX Thor includes an independent functional-safety island and support offerings extending up to ten years. **Holoscan** provides infrastructure for processing real-time sensor streams, while Sensor Bridge supports efficient sensor-to-compute paths. ([IGX](https://www.nvidia.com/en-us/edge-computing/products/igx/))

These products address deployment requirements beyond model throughput: lifecycle, integration, sensor timing, support, and system dependability.

## 8. Software on the robot: perception, motion, and runtime

### Isaac ROS

Isaac ROS supplies accelerated components within the ROS 2 ecosystem. Version **5.0**, announced in September 2026, added the ROS 2 Lyrical path and further deployment and developer integrations. ([Isaac ROS 5.0 announcement](https://blogs.nvidia.com/blog/isaac-ros-5-0-agentic-open-source-robotics/))

The principal capabilities include:

- **cuVSLAM:** visual motion estimation and localization.
- **FoundationStereo / ESS:** stereo depth estimation.
- **FoundationPose:** object-pose estimation.
- **nvblox:** reconstruction of surrounding geometry and navigation costmaps.
- **cuMotion / cuRobo:** kinematics, collision-aware planning, and trajectory optimization.
- **Nav2 and MoveIt integrations:** connections to established navigation and manipulation stacks. ([cuMotion](https://nvidia-isaac.github.io/cumotion/index.html), [nvblox](https://github.com/NVIDIA-ISAAC-ROS/isaac_ros_nvblox), [Isaac ROS releases](https://nvidia-isaac-ros.github.io/releases/index.html))

A meaningful recent architectural change is the replacement of legacy NITROS packages with native ROS message-buffer support using a CUDA backend. Older tutorials can therefore describe data paths that differ from the current release. ([Release notes](https://nvidia-isaac-ros.github.io/releases/index.html), [ROS message buffers](https://nvidia-isaac-ros.github.io/concepts/rosidl_buffer/index.html))

**Isaac ROS Deploy** packages learned policies into robot execution workflows, including ONNX/Triton paths and connections to ROS topics or `ros2_control`. Support is version-specific; an example working in one release does not establish that every hardware and simulator combination is supported. ([Isaac ROS Deploy](https://nvidia-isaac-ros.github.io/repositories_and_packages/isaac_ros_deploy/index.html))

### Isaac OS

Isaac OS is a Holoscan-based robotics runtime documented as **v0.11.0 beta**. It provides graph execution, communication, and resource management on NVIDIA software foundations. It runs above the underlying operating system. ([Isaac OS overview](https://docs.nvidia.com/isaac-os/overview))

Its application graph is explicitly declared and compiled into an execution plan. ROS 2 generally uses independently participating nodes and discovered communication relationships. Bridges allow the approaches to coexist. ([Isaac OS and ROS 2](https://docs.nvidia.com/isaac-os/explanation/isaac-os-and-ros-2))

The engineering question is which execution structure best fits the application. Timing must be measured across the full sensor-to-action path under realistic load, including contention, logging, communication, and failure recovery.

## 9. Development orchestration, fleets, and factories

NVIDIA addresses several distinct coordination problems.

### OSMO and the Physical AI Data Factory

**OSMO** orchestrates development workloads across heterogeneous infrastructure. It manages workflow specifications, dependencies, scheduling, and data inputs and outputs, with Kubernetes-based execution across cloud, on-premises, and edge resources. ([OSMO architecture](https://nvidia.github.io/OSMO/main/user_guide/architecture.html))

The **Physical AI Data Factory** blueprint connects data curation, augmentation, generation, and evaluation. Agents can help operate these workflows and troubleshoot infrastructure. ([Physical AI Data Factory announcement](https://nvidianews.nvidia.com/news/nvidia-announces-open-physical-ai-data-factory-blueprint-to-accelerate-robotics-vision-ai-agents-and-autonomous-vehicle-development))

This is useful infrastructure for repeated experiments. It does not automatically establish that a dataset is representative or that an evaluation is sufficient.

### Fleet coordination and facility simulation

**Mission Dispatch** handles robot missions through interfaces including VDA5050/MQTT and REST. **Mission Control** adds fleet-management capabilities. **cuOpt** addresses routing, scheduling, and optimization. These are different responsibilities from a robot’s immediate obstacle avoidance and motor control. ([Mission Dispatch](https://catalog.ngc.nvidia.com/orgs/nvidia/isaac/containers/mission-dispatch/-), [Mission Control](https://catalog.ngc.nvidia.com/orgs/nvidia/isaac/containers/mission-control/-), [cuOpt](https://www.nvidia.com/en-us/ai-data-science/products/cuopt/))

**Mega** is NVIDIA’s blueprint for factory-scale digital twins involving robot fleets, sensors, and operational systems. Its value is in testing how machines and workflows interact within a facility. ([Mega blueprint](https://blogs.nvidia.com/blog/mega-omniverse-blueprint/))

A robot can function correctly on its own yet create congestion or poor throughput in a fleet. These operational tools address that broader system.

## 10. Safety: Halos for Robotics

NVIDIA announced **Halos for Robotics in June 2026**, combining hardware, software, inspection capabilities, and integration blueprints. The announced elements include IGX Thor, Holoscan Sensor Bridge, Halos Core, and an Outside-In safety blueprint; some software elements were introduced through early access. Agility Robotics was an initial partner. ([Halos for Robotics announcement](https://nvidianews.nvidia.com/news/nvidia-announces-halos-for-robotics-the-industrys-first-full-stack-safety-system-for-physical-ai))

The technical architecture includes safety-related functions and mechanisms for monitoring a robot from outside its own primary perception and control path. ([Technical overview](https://developer.nvidia.com/blog/inside-nvidia-halos-for-robotics-a-full-stack-functional-safety-system-for-physical-ai/))

Three engineering questions remain separate:

| Question | What must be established |
| --- | --- |
| Does the model behave correctly? | Task performance, limits, recovery, and out-of-distribution behavior |
| Can the system reach or maintain a safe state? | Safety functions, monitoring, fault handling, and independence |
| Is the complete deployed machine acceptable? | Application-specific integration and validation |

Using a safety-capable component does not, by itself, certify the complete robot. The manufacturer and integrator still own the complete system design.

## 11. Partners, deployments, and the sectors involved

### Humanoids

NVIDIA supports competing robot manufacturers, reference designs, and research platforms.

| Company or program | Evidence of NVIDIA involvement | Maturity distinction |
| --- | --- | --- |
| **Boston Dynamics / Atlas** | Collaboration spanning Thor, Isaac Lab, and GR00T | Partnership scope should be distinguished from every production configuration |
| **Agility Robotics / Digit** | Commercial logistics deployments and an expanding NVIDIA relationship | Existing deployments and the future Digit 5 architecture are separate |
| **Figure** | NVIDIA training infrastructure, simulation, and onboard compute alongside proprietary Helix intelligence | Figure retains its own robot-model strategy |
| **Unitree / Sharpa reference humanoid** | May 2026 GR00T reference design using partner hardware | Reference platform |
| **Agile ONE SN** | October 2026 reference humanoid announcement | Early-2027 availability planned |

Boston Dynamics describes a broad development collaboration. GXO and Agility reported that Digit had moved **more than 100,000 totes** at a commercial site by October 2026. Agility separately describes Digit 5’s planned NVIDIA architecture and first-half-2027 early access. Existing tote-handling results should not be attributed to that future configuration. ([Boston Dynamics collaboration](https://bostondynamics.com/news/boston-dynamics-expands-collaboration-with-nvidia/), [GXO and Agility deployment](https://investors.gxo.com/news-releases/news-release-details/gxo-and-agility-robotics-expand-partnership-accelerate), [Agility’s NVIDIA collaboration](https://investors.agilityrobotics.com/news-releases/news-release-details/agility-robotics-expands-nvidia-collaboration-it-prepares-digit))

Figure illustrates the breadth of NVIDIA’s opportunity: its announced infrastructure uses NVIDIA while Figure develops Helix itself. Its large Nscale compute expansion includes future capacity plans, rather than an assertion that all announced capacity is already installed. ([Figure and Nscale](https://www.figure.ai/news/figure-and-nscale-sign-strategic-partnership))

The Unitree/Sharpa and Agile reference designs help standardize development configurations across models, compute, hands, and robot bodies. They remain partner-manufactured platforms. ([Open humanoid reference design](https://nvidianews.nvidia.com/news/nvidia-open-humanoid-robot-reference-design), [Isaac platform and reference humanoid announcement](https://investor.nvidia.com/news/press-release-details/2026/NVIDIA-Advances-Isaac-Platform-for-General-Purpose-Robotics-With-New-Reference-Humanoid-Frameworks-Simulation-Pipelines-and-Data/default.aspx))

### Industrial arms and manufacturing

Industrial partnerships include **ABB, FANUC, KUKA, Yaskawa, and Universal Robots**. The work spans simulation, perception, learned manipulation, compute integration, and factory workflows. ([Global robotics partnerships](https://nvidianews.nvidia.com/news/nvidia-and-global-robotics-leaders-take-physical-ai-to-the-real-world))

ABB’s announced work includes Omniverse integration through RobotStudio HyperReality, a Foxconn pilot, and evaluation of Jetson integration with OmniCore. These are concrete integration directions; their commercial progress needs to be assessed at the product and deployment level. ([ABB announcement](https://www.abb.com/global/en/news/134030/prsrl-abb-robotics-partners-with-nvidia-to-deliver-industrial-grade-physical-ai-at-scale))

Industrial robotics can adopt NVIDIA incrementally. A manufacturer might add visual perception or improve offline simulation while retaining established controllers and task programs.

### Healthcare and medical robotics

NVIDIA’s medical work includes **Isaac for Healthcare**, simulation frameworks, **GR00T-H and Cosmos-H**, and associated datasets and tooling. Some medical model weights have noncommercial restrictions even where surrounding simulation software allows commercial use. ([NVIDIA Medtech repositories](https://github.com/NVIDIA-Medtech))

GPU-native medical physics research uses technologies including Warp and Newton for interactions such as flexible instrument behavior. Hospital partnerships also include support robots and clinical-operation workflows, including Foxconn initiatives. ([Healthcare robotics and medical physics](https://developer.nvidia.com/blog/developing-healthcare-robotics-with-gpu-native-medical-physics-simulation/), [Foxconn and Taiwan medical centers](https://nvidianews.nvidia.com/news/nvidia-foxconn-and-taiwan-medical-centers-bring-agentic-and-physical-ai-to-healthy-taiwan))

These efforts cover research, sensing, simulation, and assistance. Their existence does not establish autonomous clinical capability for every demonstrated procedure.

### Other autonomous machines

NVIDIA’s reach also includes:

- **Heavy equipment:** Caterpillar’s physical-AI collaboration.
- **Drones:** onboard AI in platforms such as Skydio X10.
- **Recycling:** robotic perception and sorting systems.
- **Agriculture and outdoor robotics:** machines operating in variable, unstructured environments.
- **Entertainment and research:** work with Disney and other groups on expressive or physically capable robots.
- **Underwater and space systems:** simulation and embedded-computing applications. ([Caterpillar collaboration](https://www.caterpillar.com/en/news/corporate-press-releases/h/cat-nvidia-collab.html), [Skydio X10](https://www.skydio.com/x10), [Recycling robotics](https://blogs.nvidia.com/blog/ai-recycling-startups/), [National Robotics Week 2026](https://blogs.nvidia.com/blog/national-robotics-week-2026/), [Space computing](https://nvidianews.nvidia.com/news/space-computing))

Humanoids draw attention, but NVIDIA’s robotics exposure is distributed across many machine types and levels of autonomy.

## 12. Autonomous vehicles: the adjacent DRIVE effort

Autonomous vehicles share many of robotics’ core problems: sensing, localization, prediction, planning, simulation, onboard inference, and safety.

NVIDIA’s automotive portfolio includes **DRIVE AGX Thor**, the **Hyperion** reference architecture, driving software and models, simulation, and data-generation infrastructure. Its **Alpamayo** research and development connects reasoning models to driving. The May 2026 announcement introduced **Alpamayo 2 Super**, a 34-billion-parameter model, alongside associated development and simulation tools. ([Alpamayo 2 Super announcement](https://nvidianews.nvidia.com/news/nvidia-alpamayo-2-super-robotaxis))

**Alpamayo 2 Edge**, introduced in November, is a 2-billion-parameter model aimed at onboard compute and latency constraints. ([Onboard AV reasoning](https://developer.nvidia.com/blog/running-av-reasoning-models-within-onboard-compute-and-latency-limits/))

DRIVE belongs in a comprehensive account of NVIDIA’s embodied-AI efforts, although automotive software, commercial relationships, and validation requirements form a distinct business from warehouse or humanoid robotics.

## 13. Business model, investments, competition, and openness

### How NVIDIA can make money

The platform creates several routes to revenue:

- Training and simulation GPUs.
- Jetson and other onboard compute.
- Industrial platforms and support.
- Enterprise software and infrastructure.
- Additional compute demand generated by larger models, synthetic data, and repeated evaluation.

NVIDIA’s financial reporting does **not isolate a clean robotics-only revenue figure**. Its broader reporting categories encompass activities beyond robots, so robotics revenue should not be inferred directly from those totals. ([Fiscal 2027 second-quarter financial results](https://investor.nvidia.com/news/press-release-details/2026/NVIDIA-Announces-Financial-Results-for-Second-Quarter-Fiscal-2027/))

My assessment is that NVIDIA can benefit even when a customer chooses another company’s robot model. The infrastructure spend can remain NVIDIA-based, which broadens its opportunity beyond winning the policy-model layer.

### Investments and ecosystem development

NVentures participates in the robotics and physical-AI ecosystem, and NVIDIA is among the investors named in Figure’s financing announcements. A startup’s total funding round should not be treated as NVIDIA’s individual investment amount. ([NVentures](https://www.nvidia.com/en-us/startups/nventures/), [Figure Series C](https://www.figure.ai/news/series-c))

A significant pending transaction is NVIDIA’s **September 2026 agreement to acquire Hugging Face**. The SEC filing describes approximately **$11.9 billion payable to stockholders**, plus an employee retention program of up to approximately **$1 billion**. Closing is expected in the first half of 2027, subject to conditions. The filing also describes commitments to maintain an open platform and support other silicon vendors. ([SEC filing](https://www.sec.gov/Archives/edgar/data/1045810/000104581026000078/nvda-20260902.htm))

For robotics, Hugging Face matters through model and dataset distribution and the **LeRobot** ecosystem. As of this report, the acquisition remains pending. ([Hugging Face and LeRobot](https://blogs.nvidia.com/blog/hugging-face-lerobot-models-frameworks-open-robotics/))

### Competition

Competition exists at several layers. Google DeepMind’s **Gemini Robotics 2.5** competes in embodied intelligence, while Qualcomm’s **Dragonwing IQ10** addresses robotics compute. Robot manufacturers also develop their own intelligence and integration stacks. ([Gemini Robotics 2.5](https://deepmind.google/blog/gemini-robotics-2-5-accelerates-real-world-physical-intelligence/), [Dragonwing IQ10](https://www.qualcomm.com/internet-of-things/products/iq10-series))

The relationships can overlap: DeepMind is also a Newton collaborator. A company can cooperate on simulation, compete on models, and purchase the same hardware.

### What “open” means in this stack

| Component | Relevant licensing distinction |
| --- | --- |
| **Warp / Newton** | Open-source core projects; dependencies still need review |
| **Isaac Sim** | Source availability does not eliminate separate Kit, asset, or redistribution terms |
| **GR00T** | Apache-licensed code and separately licensed model weights |
| **Cosmos 3** | OpenMDW-1.1 source/model terms |
| **cuMotion** | License includes NVIDIA-GPU application restrictions |
| **cuVSLAM** | Underlying library has NVIDIA-platform restrictions, even where a wrapper is permissively licensed |
| **Healthcare models** | Some weights have noncommercial restrictions |

The restrictions on cuMotion, cuVSLAM, and Isaac Sim’s surrounding components are explicit in their licensing materials. ([cuMotion license](https://github.com/NVIDIA-ISAAC-ROS/isaac_ros_cumotion/blob/main/LICENSE), [cuVSLAM license](https://github.com/nvidia-isaac/cuVSLAM/blob/main/LICENSE), [Isaac Sim license FAQ](https://docs.isaacsim.omniverse.nvidia.com/6.0.0/common/license-faq.html))

Source access, permission to use commercially, permission to redistribute, and practical portability to other hardware are separate questions. Teams should evaluate them at the component level.

## 14. The limits that will determine real-world value

The unresolved challenges are as important as the product catalog.

**Data remains embodiment-dependent.** A successful demonstration does not automatically transfer across different hands, cameras, kinematics, or control interfaces.

**Simulation must capture the relevant failure modes.** Good appearance is insufficient when the task depends on friction, compliance, cables, deformable objects, or delicate contact.

**Long tasks amplify small errors.** As a simplified illustration, twenty independent steps each succeeding 99% of the time yield only about an 82% chance of completing the entire sequence without failure. Recovery and verification are therefore core capabilities.

**Compute has physical costs.** Memory capacity, power, cooling, weight, and battery life can constrain a robot even when peak arithmetic throughput looks ample.

**Integration remains substantial work.** Coordinate frames, units, normalization, timing, action semantics, calibration, and controller behavior must all agree.

**Deployment evidence is uneven.** A demonstration, research benchmark, pilot, paid deployment, and repeatable fleet rollout establish different things.

These are engineering implications of the architecture, rather than arguments that the platform lacks value. They identify what must be measured to determine whether that value reaches customers.

The most useful indicators of progress are paid deployments, intervention rates, uptime, cycle time, adaptation cost, recovery performance, and the total cost of performing a useful task.

## 15. Implications for developers building on the platform

For a software company, there is considerable room above NVIDIA’s infrastructure: experiment management, data provenance, evaluation, scenario generation, failure replay, mission orchestration, and operator interfaces.

For a custom engine, the sensible integration boundary depends on the job:

| Requirement | Plausible approach |
| --- | --- |
| Interactive authoring and inspection | Keep a custom application or browser interface |
| Shared assets and scene metadata | Use OpenUSD where appropriate |
| A focused physics capability | Evaluate native interfaces such as `ovphysx` |
| Large-scale policy training | Run dedicated Isaac or other training workers |
| Reproducible development | Version datasets, environments, policies, controllers, and evaluations together |

These are architectural recommendations. The existence of a C API for one component does not make the entire Isaac ecosystem a small native library.

A productive first implementation should target one robot and one measurable task. Establish a conventional baseline, add learning where it improves a specific limitation, and evaluate on held-out cases and physical hardware.

**Overall assessment:** NVIDIA has assembled a broad and increasingly connected foundation for robotics development. Its commercial position benefits from serving many robot makers and multiple stages of their work. The decisive test is how effectively that infrastructure helps manufacturers deliver reliable machines with acceptable economics—and how much engineering remains necessary between a successful model demonstration and a dependable deployed robot.
