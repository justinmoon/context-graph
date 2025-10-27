# Phase 1: Expected Node and Edge Extractions

Based on the original stakgraph test fixtures, here's what we need to extract:

## TypeScript Fixture (`fixtures/typescript/`)

### Files
- 5 TypeScript files (.ts)
- 1 package.json
- Expected: All `.ts` files discovered

### Node Types to Extract

1. **Repository** (1)
   - Repository root node

2. **Language** (1)
   - name: "typescript"
   - file: repository path

3. **File** (multiple)
   - package.json and all .ts files
   - body: file contents
   - name: filename

4. **Directory** (2)
   - src/
   - prisma/

5. **Import** (5)
   - One per file that has imports
   - body: All import statements concatenated
   - Must start with "import "

6. **Library** (11)
   - Extracted from package.json dependencies
   - Examples: sequelize, typeorm, express, etc.

7. **Function** (8 without LSP, 12 with LSP)
   - Named functions and arrow functions
   - Examples: log, deprecated, createPerson, getPerson
   - meta: May contain "interface" key for interface methods

8. **Class** (5)
   - Class declarations
   - Body includes methods

9. **DataModel** (10)
   - TypeScript interfaces and types used as data models
   - Sequelize models, TypeORM entities
   - Examples: User, PersonAttributes

10. **Trait** (2)
    - TypeScript interfaces (used as traits/protocols)

11. **Var** (4)
    - Top-level variable declarations
    - const, let declarations

12. **Endpoint** (2)
    - Express route definitions
    - name: route path (e.g., "/person")
    - meta["verb"]: HTTP method (GET, POST, etc.)
    - Examples: POST /person, GET /person/:id

### Edge Types to Extract

1. **Contains** (66)
   - Repository → Language
   - Repository → Directory
   - Directory → File
   - Directory → Directory (nested)
   - File → Import
   - File → Function
   - File → Class
   - File → DataModel
   - File → Var
   - Class → Function (methods)

2. **Calls** (5)
   - Function → Function (function calls within code)

3. **Imports** (12 without LSP, 15 with LSP)
   - Import → Library (from package.json)
   - Import → Function (local imports)
   - Import → Class (local imports)
   - Import → DataModel (local imports)

4. **Handler** (2)
   - Endpoint → Function (route handlers)
   - Example: POST /person endpoint → createPerson function

5. **Implements** (3)
   - Class → Trait (class implements interface)
   - DataModel → Trait

6. **Uses** (0 without LSP, 6 with LSP)
   - Cross-file type usage (requires LSP)

## React Fixture (`fixtures/react/`)

### Files
- 7 TypeScript React files (.tsx, .ts)
- 1 package.json

### Node Types to Extract

1. **Repository** (1)

2. **Language** (1)
   - name: "react"

3. **File** (multiple)
   - All .tsx, .ts files
   - package.json

4. **Directory** (3)
   - src/
   - src/components/
   - public/

5. **Import** (6)
   - One per file with imports
   - Examples: React imports, component imports, library imports

6. **Library** (18)
   - From package.json
   - Examples: react, react-dom, react-router-dom, typescript, zustand, etc.

7. **Function** (17 without LSP, 22 with LSP)
   - React components (function components, arrow components)
   - Regular functions
   - Styled components
   - Examples: App, People, NewPerson, Person, FunctionComponent, ArrowComponent
   - Hook functions: useStore

8. **Class** (1)
   - Class components (rare in modern React)
   - Example: TestThing

9. **DataModel** (4)
   - TypeScript interfaces/types for data
   - Example: Person type

10. **Var** (3)
    - Top-level constants
    - Example: API_URL

11. **Request** (2)
    - fetch() calls to backend APIs
    - meta["verb"]: HTTP method
    - meta["url"]: endpoint URL
    - Examples: POST to /person, GET to /people

12. **Page** (2)
    - React Router Route definitions
    - name: route path
    - Example: "/", "/new-person"

### Edge Types to Extract

1. **Contains** (varies)
   - Same hierarchy as TypeScript
   - File → Function (components)
   - File → Request

2. **Calls** (varies)
   - Component → Component (component usage)
   - Function → Function
   - Component → Hook (useState, useEffect, custom hooks)

3. **Imports** (varies)
   - Import → Library
   - Import → Function (component imports)

4. **Renders** (varies)
   - Page → Function (page renders component)
   - Function → Function (component renders child component)

5. **Uses** (LSP only)
   - Type usage across files

## Key Parsing Patterns

### Imports Detection
```typescript
import X from "Y"
import { A, B } from "C"
import * as D from "E"
```
- Capture entire import block per file
- Link to libraries (external) or local symbols

### Function Detection
```typescript
function name() {}
const name = function() {}
const name = () => {}
export function name() {}
export default function name() {}
```

### Component Detection (React)
```typescript
function Component() { return <div>...</div> }
const Component = () => <div>...</div>
export function Component() { return <JSX/> }
```

### Endpoint Detection
```typescript
app.post("/path", handler)
router.get("/path/:id", handler)
```

### Request Detection
```typescript
fetch("/api/endpoint", { method: "POST" })
axios.post("/endpoint")
```

### Page/Route Detection
```tsx
<Route path="/path" element={<Component />} />
```

## Implementation Priority

1. **Phase 1a: Basic Structure**
   - Repository, Language, File, Directory nodes
   - Contains edges for file hierarchy

2. **Phase 1b: Imports and Libraries**
   - Import nodes with all import statements
   - Library nodes from package.json
   - Imports edges connecting them

3. **Phase 1c: Functions and Classes**
   - Function nodes (all variants)
   - Class nodes
   - Contains edges from File → Function/Class

4. **Phase 1d: Data Models and Types**
   - DataModel nodes (interfaces, types)
   - Trait nodes (interfaces used as contracts)

5. **Phase 1e: API Patterns**
   - Endpoint nodes (Express routes)
   - Request nodes (fetch calls)
   - Handler edges (Endpoint → Function)

6. **Phase 1f: React Patterns**
   - Page nodes (Routes)
   - Renders edges (Page → Component)
   - Component usage via Calls edges

7. **Phase 1g: Cross-File Relationships**
   - Calls edges (function calls)
   - Imports edges for local imports
   - (LSP later for deeper analysis)
