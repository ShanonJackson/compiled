import { ax, ix, CC, CS } from "@compiled/react/runtime";
import { css as compiledCss, styled as compiledStyled, keyframes, ClassNames, cssMap } from '@compiled/react';
const fade = "k17e8rkr";
const toneMap = {
    primary: "_syaz101g",
    danger: "_syaz14zx"
};
const baseStyles = "_1wybdlk8 _wlt118cn _1n2z11x8";
const Wrapper = forwardRef(({ as: C = "div", style: __cmpls, ...__cmplp }, __cmplr)=>{
    if (__cmplp.innerRef) {
        throw new Error("Please use 'ref' instead of 'innerRef'.");
    }
    return <CC><CS>{[
        "._ca0qftgi{padding-top:8px}",
        "._u5f3ftgi{padding-right:8px}",
        "._n3tdftgi{padding-bottom:8px}",
        "._19bvftgi{padding-left:8px}",
        "._1l201r31:focus ._1l201r31{outline-color:currentColor}",
        "._cwctglyw:focus ._cwctglyw{outline-style:none}",
        "._d7ut1o36:focus ._d7ut1o36{outline-width:medium}"
    ]}</CS><C {...__cmplp} style={__cmpls} ref={__cmplr} className={ax([
        "_ca0qftgi",
        "_u5f3ftgi",
        "_n3tdftgi",
        "_19bvftgi",
        "_1l201r31",
        "_cwctglyw",
        "_d7ut1o36",
        __cmplp.className
    ])}/></CC>;
});
if (process.env.NODE_ENV !== "production") {
    Wrapper.displayName = "Wrapper";
}
export const Component = ()=>(<CC><CS>{[
        "._y44v65d0{animation:k17e8rkr 2s linear}",
        '._aetr1vm8:after{content:"!"}'
    ]}</CS><Wrapper className={ax([
        "_1wybdlk8 _wlt118cn _1n2z11x8",
        "_syaz101g"
    ])}>
        <span className={ax([
        "_y44v65d0 _aetr1vm8"
    ])}>
          combo
        </span>
      </Wrapper></CC>);
