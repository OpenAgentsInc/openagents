# Flow-Based Programming Glossary

* 4GL - 4th Generation Language, typically generating HLL statements
* AMPS - Advanced Modular Processing System -- first version of FBP used for production work (still in use at a major
  Canadian company)
* Applicative - describes a language which does all of its processing by means of operators applied to values
* Asynchronous - independent in time, unsynchronized
* Automatic ports - unnamed input or output ports used to respectively delay a process, or indicate termination of a
  process, without code needing to be added to the processes involved
* Brackets - IPs of a special type used to demarcate groupings of IPs within IP streams
* C#FBP - C# implementation of FBP concepts. For more information,
  see http://www.jpaulmorrison.com/fbp/index.shtml#CsharpFBP
* Capacity - the maximum number of IPs a connection can hold at one time
* Component - Reusable piece of code or reusable subnet
* Composite component - Component comprising more than one process (same as subnet)
* Connection - Path between two processes, over which a data stream passes; connections have finite capacities (the
  maximum number of IPs they can hold at one time)
* Connection Points - The point where a connection makes contact with a component
* Control IP - an IP whose life-time corresponds exactly to the lifetime of a substream, which it can be said to "
  represent"
* Coroutine - an earlier name for an FBP process
* Descriptor - read-only module which can be attached to an IP describing it to generalized components
* DFDM - Data Flow Development Manager, dialect of FBP - went on sale in Japan - sold several licenses
* DrawFBP - FBP diagramming tool, written in Java. For more information,
  see http://www.jpaulmorrison.com/fbp/index.shtml#DrawFBP
* Elementary Component - Component which is not a composite component
* FBP - Flow-Based Programming
* FPE - Flow Programming Environment - term I use in this book for the product that was to follow DFDM. It was developed
  quite far theoretically, but never reached the marketplace
* Granularity - "Grain" size of components
* Higher-Level Language (HLL) - a language intermediate in level between Lower-Level Languages (e.g. Assembler) and 4th
  Generation Languages (4GLs)
* Information Packet (IP) - an independent, structured piece of information with a well-defined lifetime (from creation
  to destruction)
* Initial Information Packet (IIP) - data specified in the network definition, usually used as a parameter for a
  reusable component; it is converted into a "real" IP by means of a "receive" service call
* JavaFBP - Java implementation of FBP concepts. For more information,
  see http://www.jpaulmorrison.com/fbp/index.shtml#JavaFBP
* Looper - a component which does not exist after each IP has been handled, but "loops" back to get another one
* Non-looper - a component which exits after each IP has been handled, rather than "looping" back to get another one
* Port - The point where a connection makes contact with a process
* Process - Asynchronously executing piece of logic -- in FBP, same as "thread"
* Root IP - The root of a tree of IPs
* Stream - Sequence of IPs passing across a given connection
* Substream-sensitivity - a characteristic of some ports of a composite component where brackets are treated as end of
  data
* Synchronous - Coordinated in time (at the same time)
* Thread - Same as "process" in FBP -- often referred to as "lightweight" process
* THREADS - C (or C++)-based FBP implementation. For more information,
  see http://www.jpaulmorrison.com/fbp/index.shtml#THREADS
* Tree - In FBP, acyclic structure of linked IPs, able to be sent and received as a single unit
* WYSIWYG - "What You See Is What You Get" (describes a tool where the image shown to the developer closely matches the
  final result in appearance)