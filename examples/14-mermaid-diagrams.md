# Mermaid Diagrams

Real-world mermaid diagrams from open source projects and documentation.

## Kubernetes Pod Creation

From the [Kubernetes contributors blog](https://www.kubernetes.dev/blog/2021/12/01/improve-your-documentation-with-mermaid.js-diagrams/), showing the full lifecycle of creating a pod.

```mermaid
sequenceDiagram
    actor me
    participant apiSrv as control plane<br><br>api-server
    participant etcd as control plane<br><br>etcd datastore
    participant cntrlMgr as control plane<br><br>controller<br>manager
    participant sched as control plane<br><br>scheduler
    participant kubelet as node<br><br>kubelet
    participant container as node<br><br>container<br>runtime
    me->>apiSrv: 1. kubectl create -f pod.yaml
    apiSrv-->>etcd: 2. save new state
    cntrlMgr->>apiSrv: 3. check for changes
    sched->>apiSrv: 4. watch for unassigned pods(s)
    apiSrv->>sched: 5. notify about pod w nodename=" "
    sched->>apiSrv: 6. assign pod to node
    apiSrv-->>etcd: 7. save new state
    kubelet->>apiSrv: 8. look for newly assigned pod(s)
    apiSrv->>kubelet: 9. bind pod to node
    kubelet->>container: 10. start container
    kubelet->>apiSrv: 11. update pod status
    apiSrv-->>etcd: 12. save new state
```

## Kubernetes Ingress

From the [Kubernetes official docs](https://kubernetes.io/docs/concepts/services-networking/ingress/), showing Ingress routing with custom styling.

```mermaid
graph LR;
 client([client])-. Ingress-managed <br> load balancer .->ingress[Ingress];
 ingress-->|routing rule|service[Service];
 subgraph cluster
 ingress;
 service-->pod1[Pod];
 service-->pod2[Pod];
 end
 classDef plain fill:#ddd,stroke:#fff,stroke-width:4px,color:#000;
 classDef k8s fill:#326ce5,stroke:#fff,stroke-width:4px,color:#fff;
 classDef cluster fill:#fff,stroke:#bbb,stroke-width:2px,color:#326ce5;
 class ingress,service,pod1,pod2 k8s;
 class client plain;
 class cluster cluster;
```

## Terraform AWS Infrastructure

From [RoseSecurity/Terramaid](https://github.com/RoseSecurity/Terramaid), auto-generated from Terraform files showing real AWS resource dependencies.

```mermaid
flowchart TD
	subgraph Terraform
		subgraph Aws
			aws_db_instance.main_db["aws_db_instance.main_db"]
			aws_instance.app_server["aws_instance.app_server"]
			aws_instance.web_server["aws_instance.web_server"]
			aws_lb.web["aws_lb.web"]
			aws_lb_listener.web["aws_lb_listener.web"]
			aws_lb_target_group.web["aws_lb_target_group.web"]
			aws_lb_target_group_attachment.web["aws_lb_target_group_attachment.web"]
			aws_s3_bucket.logs["aws_s3_bucket.logs"]
			aws_s3_bucket.test["aws_s3_bucket.test"]
			aws_s3_bucket_policy.logs_policy["aws_s3_bucket_policy.logs_policy"]
			aws_s3_bucket_policy.test_policy["aws_s3_bucket_policy.test_policy"]
			aws_security_group.db["aws_security_group.db"]
			aws_security_group.web["aws_security_group.web"]
			aws_subnet.private["aws_subnet.private"]
			aws_subnet.public["aws_subnet.public"]
			aws_vpc.main["aws_vpc.main"]
		end
		aws_lb.web --> aws_security_group.web
		aws_lb.web --> aws_subnet.public
		aws_lb_listener.web --> aws_lb.web
		aws_lb_listener.web --> aws_lb_target_group.web
		aws_lb_target_group.web --> aws_vpc.main
		aws_lb_target_group_attachment.web --> aws_instance.web_server
		aws_lb_target_group_attachment.web --> aws_lb_target_group.web
		aws_s3_bucket_policy.logs_policy --> aws_s3_bucket.logs
		aws_s3_bucket_policy.test_policy --> aws_s3_bucket.test
		aws_security_group.db --> aws_security_group.web
		aws_security_group.web --> aws_vpc.main
		aws_subnet.private --> aws_vpc.main
		aws_subnet.public --> aws_vpc.main
	end
```

## GitLab CI/CD with ArgoCD Pipeline

From a [production deployment guide](https://blog.jklug.work/posts/argocd-gitlab-pt1/), showing the full flow from developer push through CI to Kubernetes.

```mermaid
graph TD
    Dev(Developer) -.->|Push changes| A1

    subgraph A[Code Repository]
        A1[Source Code & Dependencies] -.->|Trigger pipeline| A3[CI Pipeline Manifest]
        A2[Dockerfile]
    end

    A3 -.-> B

    subgraph B[GitLab CI Pipeline]
        B1(Stage 1: build_image) -.->|Job dependency| B2(Stage 2: update_helm_chart)
    end

    B1 -.-> BuildStageGroup
    subgraph BuildStageGroup["Build Stage"]
        S3A("Login to GitLab Container Registry") -.-> S3B("Build image from Dockerfile")
        S3B -.-> S3C("Push image to GitLab Container Registry")
    end

    S3B -.-> D1
    D2 -.->|Containerized Application| S3C
    subgraph Dockerfile["Dockerfile"]
        D1("Build Image") -.->|Compiled Source| D2("Runtime Image")
    end

    A1 -.-> D1
    B2 -.->|Update values| C2
    A2 -.-> Dockerfile
    S3C -.->|Push image| Registry(GitLab Registry)

    subgraph C[Helm Chart Repository]
        C1[Templates]
        C2[values.yaml]
    end

    Argo(Argo CD) -.->|Sync changes| C
    Argo(Argo CD) -.->|Deploy changes| K8s(Kubernetes Cluster)
    K8s -.->|Pull image| Registry
```

## OAuth 2.0 Authorization Flow

Standard OAuth 2.0 authorization code grant, from a widely-referenced [gist](https://gist.github.com/cseeman/cf1a0cf7d931794d78f570e9f413f4a1).

```mermaid
sequenceDiagram
    participant C as Client
    participant O as Resource Owner
    participant A as Authorization Server
    participant R as Resource Server

    C->>O: requests authorization
    O->>C: receives authorization grant
    C->>A: requests access token, presents grant
    A->>C: authenticates client, validates grant, issues access token
    C->>R: requests protected resource, presents access token
    R->>C: validates access token, serves request
```

## Scaled System Architecture

From [rudolfolah/mermaid-diagram-examples](https://github.com/rudolfolah/mermaid-diagram-examples), a financial aggregation app architecture showing DNS, CDN, load balancing, API layer, message queues, microservices, and storage tiers.

```mermaid
graph TB
Client --> DNS & CDN & lb[Load Balancer]
lb --> web[Web Server]
subgraph api
web --> accounts[Accounts API] & read[Read API]
memoryCache[Memory Cache]
end

accounts --> queue[Queue] --> tes

subgraph storage
dbPrimary[(SQL Write Primary)] -.- dbReplica[(SQL Read Replicas)]
objectStore[(Object Store)]
end

subgraph services
tes[Transaction Extraction Service] --> category[Category Service] & budget[Budget Service] & notif[Notification Service]
end

tes --> objectStore
CDN --> objectStore
tes --> dbPrimary
accounts --> dbPrimary & dbReplica
read --> dbReplica & memoryCache[Memory Cache]
```

## Django Watson Class Diagram

From [rudolfolah/mermaid-diagram-examples](https://github.com/rudolfolah/mermaid-diagram-examples), documenting the real architecture of the `django-watson` full-text search library.

```mermaid
classDiagram
class SearchAdapter {
  fields
  exclude
  store
  __init__(model)
  prepare_content(content)
  get_title(obj)
  get_description(obj)
  get_content(obj)
  get_url(obj)
  get_meta(obj)
  serialize_meta(obj)
  deserialize_meta(obj)
  get_live_queryset()
}

class SearchContextManager {
  _stack
  __init__()
  is_active()
  start()
  add_to_context(engine, obj)
  invalidate()
  is_invalid()
  end()
  update_index()
  skip_index_update()
}

class SearchEngine {
  list _created_engines$
  dict _registered_models
  str _engine_slug
  SearchContextManager _search_context_manager
  is_registered(model) bool
  register(model, adapter_cls)
  unregister(model)
  get_registered_models() list
  get_adapter(model) SearchAdapter
  update_obj_index(obj)
  search(search_text, models) Queryset
  filter(queryset, search_text) Queryset
}

SearchContextManager *-- SearchContext
SearchContext <|-- SkipSearchContext
```

## Pie Chart

```mermaid
pie showData
    title npm Downloads by Package Manager
    "npm" : 65
    "yarn" : 20
    "pnpm" : 12
    "bun" : 3
```

## State Diagram

```mermaid
stateDiagram-v2
    [*] --> Draft
    Draft --> InReview : Submit PR
    InReview --> ChangesRequested : Request changes
    ChangesRequested --> InReview : Push fixes
    InReview --> Approved : Approve
    Approved --> Merged : Merge
    Merged --> [*]
    InReview --> Closed : Close
    Closed --> [*]
```

## Git Graph

```mermaid
gitGraph
    commit
    commit
    branch develop
    checkout develop
    commit
    commit
    branch feature
    checkout feature
    commit
    commit
    checkout develop
    merge feature
    commit
    checkout main
    merge develop tag: "v1.0.0"
    commit
    branch hotfix
    checkout hotfix
    commit
    checkout main
    merge hotfix tag: "v1.0.1"
```
