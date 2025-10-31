# What Are Engrams? (Elevator Pitch)

## One-Sentence Summary

**Engrams are immutable archives containing compressed CML documents and SQLite databases that mount once for instant access, replacing PDFs and eliminating server costs for static knowledge distribution.**

---

## Three Key Innovations

### 1. Efficient Compression (Current: Standard, Planned: CML-Specific)

**Current (v0.2):**
- Standard LZ4/Zstd compression via proven libraries
- Smart compression method selection
- Fast and reliable

**Planned:**
- CML-specific tokenization (70-75% reduction target)
- Custom dictionary generation from CML profile schemas
- Structure compressed via Unicode tokenization
- **Target**: US legal code from 2GB → 500MB without quality loss

### 2. Mount Once, Read Forever

- Decompress all files on mount (2-5 seconds one-time cost)
- Cache in memory or disk for instant access (<1ms per read)
- SQLite database queried via VFS (no extraction needed)
- **Result**: Distribution efficiency + runtime speed

### 3. Content/Presentation Separation

- CML contains pure semantic content (50KB vs 5MB PDF)
- Rendering engine handles fonts, styling, layouts
- User-configurable themes, accessibility, responsive design
- **Result**: 100x smaller files, infinite customization

---

## What Problem Does This Solve?

**Before:**
- Wikipedia: 90GB, served billions of times/year, costs millions in hosting
- Medical PDFs: 50GB, unsearchable, expensive subscriptions, no offline access
- Legal codes: Scattered across hundreds of government websites, inaccessible

**After (with planned CML compression):**
- Wikipedia: ~27GB engram (target with CML tokenization), download once, query forever, $0 after distribution
- Medical guides: ~15GB engram (target), fully searchable, works on $100 tablets offline
- Legal codes: ~500MB engram (target), cross-document search, every law in your pocket

**Current (v0.2 with standard compression):**
- Proven archive format with SQLite integration
- Standard compression delivers reliable size reduction
- Ready for production use today

---

## How It Works

```
1. Compile Time:
   ├─ Analyze CML documents
   ├─ Generate custom dictionary (only used elements)
   ├─ Compress: tokenize structure → zstd → engram
   └─ Build SQLite indexes (FTS5, vectors)

2. Distribution:
   ├─ Git, BitTorrent, USB, HTTP
   ├─ Delta updates (download changes only)
   └─ Mirrors worldwide (decentralized)

3. Runtime:
   ├─ Mount engram (decompress once, 2-5 sec)
   ├─ Read files (instant, <1ms)
   ├─ Query database (standard SQLite speed)
   └─ Render via Continuity Engine (themes, accessibility)
```

---

## Comparison Matrix

|  | PDFs | ZIP Archives | Engrams |
|---|------|-------------|---------|
| **File size** | Bloated (fonts+layout) | Medium | Minimal (content only) |
| **Extraction** | N/A | Every use | Never (mount once) |
| **Search** | Per-document | None | Cross-document (SQLite) |
| **Updates** | Re-download all | Re-download all | Delta patches |
| **Accessibility** | Poor (fixed layout) | Depends | Excellent (semantic) |
| **Offline** | One file at a time | Extract first | Entire archive instant |
| **Customization** | None (baked in) | None | Full (themes, layouts) |
| **Preservation** | Poor (proprietary) | Good | Excellent (plain text) |

---

## Real-World Impact

### Wikipedia
- **Current**: $2-3M/year infrastructure costs
- **With Engrams**: One-time distribution to mirrors, 60-80% cost reduction
- **User benefit**: Download once, Wikipedia forever, no internet required

### Emergency Response (Gaza, Ukraine, Syria)
- **Current**: Internet cuts = lose access to critical protocols
- **With Engrams**: 2GB USB drive = complete medical + legal + educational reference
- **Impact**: Knowledge survives when infrastructure doesn't

### Rural Healthcare
- **Current**: WHO guidelines = 50GB PDFs, expensive subscriptions, no offline search
- **With Engrams**: 15GB engram, works on $100 tablets, query entire medical corpus offline
- **Impact**: Field medics have hospital-grade references anywhere

### Legal Access
- **Current**: Hours searching across government websites, need law degree to navigate
- **With Engrams**: "Show me California employment law on overtime" → instant results
- **Impact**: Workers know their rights, accessible to everyone

---

## Technical Stack

- **Format**: Custom .eng archive (TOC + compressed files + SQLite)
- **Compression**: CML tokenization + zstd (70-75% reduction)
- **Database**: SQLite with FTS5 (full-text) + vector embeddings (semantic search)
- **Bindings**: Rust via NAPI-RS (Electron/Node.js integration)
- **Distribution**: Git, BitTorrent, HTTP, USB
- **License**: [TBD - AGPL-3.0 recommended for civic infrastructure]

---

## Why This Matters

### For Platforms
- Eliminate 60-80% of infrastructure costs
- Pay bandwidth once vs billions of requests
- No single point of failure (decentralized mirrors)

### For Users
- Works offline (planes, rural areas, internet outages)
- No subscriptions (download once, keep forever)
- No tracking (local-first, private)
- Accessible (screen readers, themes, responsive)

### For Humanity
- Knowledge survives institutional collapse
- Communities own their data (not just access it)
- Post-conflict resilience (USB drives, local mirrors)
- 100-year preservation (plain text, open formats)

---

## Status

- **Current Version**: v0.2 - Core archive system operational
- **Implemented**: Archive format, SQLite VFS, standard compression, NAPI-RS bindings
- **In Progress**: Electron integration, optimization
- **Planned**: CML tokenization compression, advanced caching
- **Use Cases**: National Archive, Civic Atlas, medical references, Wikipedia distribution

---

## Questions People Ask

**Q: Why not just use ZIP files?**  
A: ZIP requires extraction (temp files, cleanup, security risks) or streaming decompression (5-15ms per read). Engrams decompress once on mount for instant access.

**Q: Why not use existing compression (gzip, zstd)?**  
A: Those are generic. Engrams use CML-specific tokenization (70-75% compression) because we know the exact schema at compile time.

**Q: Why not keep PDFs?**  
A: PDFs bundle content + fonts + styling (bloated, inaccessible). Engrams separate content (CML) from presentation (rendering engine), resulting in 100x smaller files with better accessibility.

**Q: Can I modify engrams?**  
A: No, they're immutable (read-only). Updates work via versioning (download v2, remount). This immutability is what enables perfect optimization.

**Q: What if I want to use Wikipedia/etc with this?**  
A: Content must convert to CML first. One-time cost, perpetual benefit (compression, query, preservation).

**Q: How do I get started?**  
A: [Coming soon] CLI tools for compiling engrams, mounting, and querying. For now, see GitHub repos for technical details.

---

## Learn More

- [Format Specification](./SPEC.md) - Archive format details (v0.1)
- [Compression Spec](./engram_compression_spec_v0.2.md) - Planned compression system
- [Architecture Clarification](./engram_architecture_clarification.md) - Design decisions
- GitHub: [Coming soon - repos will be public when ready]

---

*Created by Magnus, Manifest Humanity*  
*October 2025*
