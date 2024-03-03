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
