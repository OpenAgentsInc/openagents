## Core concepts

## Core concepts

- **Information Packet (IP)**: A discrete data unit managed via abstract references, enabling modular data manipulation
  across concurrent application components. IPs vary in size but are uniformly handled, promoting efficiency and
  modularity in data flow.

- **Initial Information Packets (IIPs)**: Static data pre-assigned to certain ports before execution starts. IIPs
  provide initial conditions or configuration data to processes, facilitating customizable and flexible system setups.

- **Process**: A component within FBP that performs specific tasks on IPs. Processes run concurrently, allowing
  multitasking and parallel data processing. They can be suspended and resumed without awareness, enhancing system
  flexibility.

- **Non-Deterministic Processor (NDP)**: A type of process that may produce different outputs from the same inputs on
  different executions. NDPs add variability and adaptability to the system, enabling more dynamic data processing.

- **Port**: The interface point for connecting processes to FIFO queues. Ports serve as named entry and exit points for
  data (IPs), structuring interactions between processes and facilitating data flow.

- **FIFO Queue**: A first-in, first-out structure that manages the flow of IPs between processes. It acts as a buffer,
  ensuring orderly and controlled data transmission.

- **Graph**: Represents the network of processes interconnected through ports and FIFO queues. It visualizes the
  application's architecture, aiding in design, understanding, and modification of the data flow.

- **Subgraphs**: Defined segments of the graph that encapsulate a collection of interconnected processes and queues.
  Subgraphs can be treated as single units (or black boxes) within larger graphs, simplifying complex systems and
  improving manageability.

- **Black Box**: A process or subgraph that is encapsulated, revealing only its inputs and outputs. This abstraction
  promotes modularity and reusability by concealing internal complexity.

- **Service Requests**: Actions by processes to manage IPs, including "receive", "send", and "drop". These requests are
  fundamental to controlling the flow, reception, transmission, and disposal of data.

- **Back-Pressure**: A mechanism that regulates the data transmission rate between processes to prevent overload. It
  ensures system stability and efficiency by avoiding data congestion and potential loss.

- **Synchronization**: The coordination mechanism that ensures processes operate in harmony, particularly in systems
  where timing and order of operations are critical. Synchronization techniques manage dependencies and sequence flows,
  ensuring coherent system behavior.
