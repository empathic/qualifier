# Call to action
Someone dropped 30,000 lines of slopcode in your lap and now you need to figure
out if it does what it says on the tin?

Qualifier is here to help!

Qualifier is a deterministic system and format to record quality attestations
and blockers.

Would this test be amazing if it just did one more thing?  Qualify it!  Feel
like this module is absolutely $#!? and need Claude to rewrite it?  Qualify it!

Qualifier provides an in-repo format for not only tracking code quality at scale
while you run toward that deadline, but gives a clear way to improve the system
as you go (or when you have the time again!).

(Mascot is rhe Koalafier climbing a qualifier flag and nibbling it)

# Thoughts on implementation
* .qual files are VCS-friendly JSONL qualifier attestations, suggested fixes, etc.
* The qualifier provides a human and agent-friendly interface for updating .qual
  files
* qualifier allows calculation of a "relative" quality score given a code graph
  (qualifier has an interface format that can be generated from dependency
  tracking tools like bazel, etc.)
* qualifier is intended to be integrated with code review tools, editors, coding
  agents, etc. both for "qualifying" code and for determining how best to
  improve code.
* The quality for a particular artifact is dependent on a function of the
  quality of its inputs, e.g. if a binary that has a high quality score suddenly
  depends on a library that has a low quality score, the binary's quality score
  will drop.
