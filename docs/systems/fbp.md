Excerpts and paraphrases from Flow-Based Programming, 2nd Edition by J. Paul Morrison

```mermaid
graph LR
    A([ ]) -->|IN| B([Filter])
    B -->|OUT| C([ ])
```

```mermaid
graph LR
    A[Data Source] -->|Data| B((Processor 1))
    B --> C((Processor 2))
    C --> D((Processor 3))
    D --> E{Decision}
    E -->|True| F[Result 1]
    E -->|False| G[Result 2]
```

```mermaid
graph TD
    Agent -->|executes| Run
    Run -->|instance of| Flow
    Flow -->|sequence of| Node
    Node -->|connected by| Port
    Node -->|can be| Plugin
    Plugin -->|extends| Node
    Node -->|references| File
    Agent -->|participates in| Thread
    Thread -->|comprises| Message
    Agent -->|uses| Node
    Node -->|may have| Fee
    Fee -->|payable to| Creator

classDef classDefault fill:#f9f,stroke:#333,stroke-width:2px;
class Agent,Thread,Message,File,Run,Flow,Node,Port,Plugin,Fee,Creator classDefault;

```