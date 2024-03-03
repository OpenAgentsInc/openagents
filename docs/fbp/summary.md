## Core concepts

* **Information Packet (IP)** - A discrete data unit managed via abstract references. IPs enable modular data
  manipulation across concurrent application components, akin to items on an assembly line. They vary in size but are
  uniformly handled, promoting data flow efficiency and modularity.
* **Process** - A component within Flow-Based Programming that performs specific tasks on IPs. Processes run
  concurrently, sharing machine time and can be suspended and resumed without being aware, allowing for multitasking and
  parallel data processing.
* **FIFO Queue** - A first-in, first-out structure managing the flow of IPs between processes. It acts as a buffer with
  a defined capacity, ensuring orderly and controlled data transmission.
* **Port** - The interface point where processes connect to FIFO queues. Ports serve as named entry and exit points for
  data (IPs), facilitating structured interaction between processes and data flow.
* **Service Requests** - Actions by processes to manage IPs, including "receive", "send", and "drop". These requests
  control data flow, reception, transmission, and disposal of data.
* **Back-Pressure** - A flow control mechanism that regulates the rate of data transmission between processes to prevent
  overload. When a receiving process is unable to process incoming Information Packets (IPs) as quickly as they are
  sent, back-pressure signals the sending process to slow down, ensuring that the system operates within its capacity
  limits. This mechanism helps maintain stability and efficiency in the system by avoiding data congestion and potential
  loss.
* **Graph** - Represents the network of processes interconnected through ports and FIFO queues, defining the flow of IPs
  within the system. The graph visualizes the application's architecture, making it easier to design, understand, and
  modify the data flow.
* **Black Box** - A process or a subgraph within the system that is encapsulated and only exposes its inputs and
  outputs, hiding its internal complexity. This promotes modularity and reusability by allowing developers to integrate
  black boxes without knowing their internal workings.
* **Non-Deterministic Processor (NDP)** - A type of process that can produce different outputs given the same set of
  inputs in different executions. NDPs introduce variability and adaptability into the system, allowing for more
  flexible and dynamic data processing.
