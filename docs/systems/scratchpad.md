Excerpts and paraphrases from Flow-Based Programming, 2nd Edition by J. Paul Morrison

```mermaid
graph LR
    A([ ]) -->|IN| B([Filter])
    B -->|OUT| C([ ])
```

```mermaid
%%{init: {'themeVariables': { 'primaryColor': '#A7A7A7', 'lineColor': '#8B8585', 'mainBkg': '#1e1e1e', 'nodeBkg': '#2C2C2D', 'nodeBorder': '#A7A7A7', 'clusterBkg': '#3D3D40', 'clusterBorder': '#8B8585', 'textColor': '#A7A7A7', 'edgeLabelBackground':'#1e1e1e', 'fontSize': '16px', 'fontFamily': '"JetBrains Mono", monospace'}}}%%
graph TD
    Agent((Agent)) -->|executes| Run((Run))
    Run -->|instance of| Flow((Flow))
    Flow -->|sequence of| Node((Node))
    Node -->|connected by| Port((Port))
    Node -->|can be| Plugin((Plugin))
    Plugin -->|extends| Node
    Node -->|references| File((File))
    Agent -->|participates in| Thread((Thread))
    Thread -->|comprises| Message((Message))
    Agent -->|uses| Node
    Node -->|may have| Fee((Fee))
    Fee -->|payable to| Creator((Creator))

classDef classDefault fill:#2C2C2D,stroke:#A7A7A7,stroke-width:2px,color:#A7A7A7;
class Agent,Thread,Message,File,Run,Flow,Node,Port,Plugin,Fee,Creator classDefault;

```