import * as React from "react";
import { ax, ix, CC, CS } from "@compiled/react/runtime";
const _2 = "._syaz13q2{color:blue}";
const _ = "._syaz5scu{color:red}";
const styles = {
  primary: "_syaz5scu",
  secondary: "_syaz13q2",
};
<CC>
  <CS>{[_, _2]}</CS>
  {<Component xcss={isPrimary ? styles.primary : styles.secondary} />}
</CC>;
