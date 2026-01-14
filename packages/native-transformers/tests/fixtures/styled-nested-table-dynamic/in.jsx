import { styled } from '@compiled/react';

type Props = {
  enabled: boolean;
};

export const Component = styled.div<Props>({
  'table[id^="tt_apb_graph_outer_"]': (props) =>
    props.enabled
      ? {
          width: '80px',
          td: {
            paddingBlock: 0,
          },
          tbody: {
            border: 0,
          },
          img: {
            display: 'block',
          },
          'td.tt_spacer': {
            padding: 0,
            minWidth: '1px',
          },
          'td.tt_graph_percentage': {
            minWidth: 'auto',
            textAlign: 'right',
            paddingRight: '3px',
            width: '3em',
          },
          'img.hideOnPrint': {
            border: 0,
            height: '4px',
            width: '100%',
          },
        }
      : {
          td: {
            paddingBlock: 0,
          },
          tbody: {
            border: 0,
          },
          img: {
            display: 'block',
          },
          'td.tt_spacer': {
            padding: 0,
          },
          'td.tt_graph_percentage': {
            minWidth: 'auto',
          },
        },
});

export const Example = () => <Component enabled />;
